//! SQLite storage, offline-first. Every synced row has `updated_at` (ms since
//! the epoch) and `deleted`, for last-write-wins sync (see `sync`).
//!
//! What is stored:
//! - `messages`: each bank or wallet SMS as its *parsed* fields (JSON of
//!   [`Parsed`]); the raw text is not kept.
//! - `txn_meta`: a category or note set by hand, per transaction id, and
//!   which earlier transactions it repays (see `settle`).
//! - `recipient_rules`: "always use this category for this recipient".
//! - `settings`: JSON values (`plans`, `settings`); keys starting with
//!   `local.` stay on this device.
//! - `unrecognized` (this device only, never synced): messages from a known
//!   sender that mention money but match no rule, kept with their text so a
//!   new format can be added.
//!
//! Transactions aren't stored: they are worked out from the messages each
//! time ([`reconcile`]), so a better rule or a fixed bug applies to the past too.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{FixedOffset, TimeZone};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};

use crate::parse::{Kind, Outcome, Parsed, Parser, Sms};
use crate::reconcile::{reconcile, txn_id, Flow, Txn};
use crate::settle::{self, Suggestion};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS messages (
    id          TEXT PRIMARY KEY,      -- hash of sender, receive time and text
    received_at INTEGER NOT NULL,      -- ms since the epoch
    parsed      TEXT NOT NULL,         -- JSON of finco_core::Parsed
    updated_at  INTEGER NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS txn_meta (
    id          TEXT PRIMARY KEY,      -- finco_core::Txn id
    category    TEXT,                  -- NULL: automatic
    note        TEXT,
    settles     TEXT,                  -- JSON list of txn ids this repays
    updated_at  INTEGER NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS recipient_rules (
    key         TEXT PRIMARY KEY,      -- see recipient_key()
    category    TEXT NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS unrecognized (
    id          TEXT PRIMARY KEY,
    sender      TEXT NOT NULL,
    body        TEXT NOT NULL,
    received_at INTEGER NOT NULL,
    reason      TEXT NOT NULL
);
"#;

pub struct Store {
    pub(crate) conn: Connection,
}

pub(crate) fn stamp() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Messages are dated in Addis Ababa time.
pub fn tz() -> FixedOffset {
    FixedOffset::east_opt(3 * 3600).unwrap()
}

/// Stable id for one SMS, so reading the inbox twice stores it once.
pub fn message_id(sender: &str, received_ms: i64, body: &str) -> String {
    // FNV-1a, 64-bit: no need for a cryptographic hash here.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in sender.trim().bytes().chain(received_ms.to_le_bytes()).chain(body.bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// Who a payment went to, for learning categories: the account (or merchant
/// code), else the name, else the number a package was for. A bank debit
/// with no detail has no key and is never auto-tagged.
pub fn recipient_key(t: &Txn) -> Option<String> {
    if matches!(t.kind, Kind::Package | Kind::Airtime) {
        return t.target_number.as_ref().map(|n| format!("number:{n}"));
    }
    if let Some(a) = &t.counterparty_account {
        return Some(format!("{}:acct:{a}:{}", t.source, t.counterparty.as_deref().unwrap_or("").to_lowercase()));
    }
    t.counterparty.as_ref().map(|c| format!("{}:name:{}", t.source, c.to_lowercase()))
}

/// A transaction as the app shows it.
#[derive(Debug, Clone, Serialize)]
pub struct TxnView {
    #[serde(flatten)]
    pub txn: Txn,
    pub note: Option<String>,
    /// Ids of earlier transactions this one repays.
    pub settles: Vec<String>,
    /// Paid back so far by later transactions.
    pub repaid: i64,
    /// What counts toward spending or income: the amount minus repayments,
    /// 0 for repayments, lent and borrowed money (and for transfers between
    /// your own accounts and loans, which never count).
    pub counted: i64,
}

/// What happened to one incoming SMS.
#[derive(Debug, Clone, Serialize)]
pub struct Ingested {
    /// `parsed`, `duplicate`, `ignored`, `unrecognized` or `unknown_sender`.
    pub outcome: &'static str,
    /// The transaction it became, when it's spending with no category yet:
    /// the one to ask "what was this?" about.
    pub ask: Option<TxnView>,
    /// When it's money in that probably pays back an earlier payment: the
    /// money in, and that payment.
    pub repay: Option<(TxnView, Suggestion)>,
}

/// User settings the core needs (the app keeps more in the same JSON).
struct CoreSettings {
    own_numbers: Vec<String>,
    amount_rules: HashMap<i64, String>,
}

pub fn default_settings() -> Value {
    json!({
        "own_numbers": [],
        "amount_rules": { "1500": "transport", "2000": "transport", "2500": "transport", "5000": "transport" },
    })
}

pub fn default_plans() -> Value {
    json!({
        "salary": 0,
        "save_percent": 20,
        "bills": [
            { "id": "rent", "name": "Rent", "category": "rent", "amount": 0 },
            { "id": "electricity", "name": "Electricity", "category": "electricity", "amount": 0 },
        ],
        "bills_paid": {},
        "weekly": {},
        "goal": null,
    })
}

/// Shallow merge: keys missing from `stored` come from `defaults`.
fn with_defaults(stored: Option<Value>, defaults: Value) -> Value {
    let mut out = defaults;
    if let (Some(Value::Object(s)), Value::Object(o)) = (stored, &mut out) {
        for (k, v) in s {
            o.insert(k, v);
        }
    }
    out
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
        // The UI, the SMS receiver and sync open it at the same time.
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(SCHEMA).context("creating schema")?;
        // Added after the first release.
        let has_settles: bool =
            conn.query_row("SELECT COUNT(*) FROM pragma_table_info('txn_meta') WHERE name = 'settles'", [], |r| r.get::<_, i64>(0))? > 0;
        if !has_settles {
            conn.execute_batch("ALTER TABLE txn_meta ADD COLUMN settles TEXT")?;
        }
        conn.execute_batch(&crate::sync::schema_sql()).context("creating sync tables")?;
        Ok(Self { conn })
    }

    /// Run `f` in one transaction.
    pub fn with_tx<T>(&self, f: impl FnOnce(&Self) -> Result<T>) -> Result<T> {
        let tx = self.conn.unchecked_transaction()?;
        let out = f(self)?;
        tx.commit()?;
        Ok(out)
    }

    // ── Messages ────────────────────────────────────────────────────────────

    /// Parse and store one SMS. Safe to call twice for the same message.
    pub fn ingest(&self, parser: &Parser, sender: &str, body: &str, received_ms: i64) -> Result<Ingested> {
        let received_at = tz().timestamp_millis_opt(received_ms).single().context("bad receive time")?;
        let id = message_id(sender, received_ms, body);
        let outcome = parser.parse(&Sms { sender, body, received_at });
        // Understood now (a rule was added since): no longer "not understood".
        if matches!(outcome, Outcome::Parsed(_) | Outcome::Ignored { .. }) {
            self.conn.execute("DELETE FROM unrecognized WHERE id = ?1", [&id])?;
        }
        let parsed = match outcome {
            Outcome::Parsed(p) => p,
            Outcome::Ignored { .. } => return Ok(Ingested { outcome: "ignored", ask: None, repay: None }),
            Outcome::UnknownSender => return Ok(Ingested { outcome: "unknown_sender", ask: None, repay: None }),
            Outcome::Unrecognized { reason } => {
                self.conn.execute(
                    "INSERT OR IGNORE INTO unrecognized (id, sender, body, received_at, reason) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id, sender.trim(), body, received_ms, reason],
                )?;
                return Ok(Ingested { outcome: "unrecognized", ask: None, repay: None });
            }
        };
        let inserted = self.conn.execute(
            "INSERT OR IGNORE INTO messages (id, received_at, parsed, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![id, received_ms, serde_json::to_string(&parsed)?, stamp()],
        )?;
        if inserted == 0 {
            return Ok(Ingested { outcome: "duplicate", ask: None, repay: None });
        }
        let want = txn_id(&parsed);
        let all = self.transactions()?;
        let Some(t) = all.iter().find(|t| t.txn.id == want) else {
            return Ok(Ingested { outcome: "parsed", ask: None, repay: None });
        };
        let ask = (t.txn.flow == Flow::Spend && t.txn.category.is_none()).then(|| t.clone());
        // Money in from a person: maybe someone paying you back.
        let repay = (t.txn.flow == Flow::Income && t.txn.kind == Kind::TransferIn)
            .then(|| settle::suggest(&all, t).into_iter().find(|s| s.likely))
            .flatten()
            .map(|s| (t.clone(), s));
        Ok(Ingested { outcome: "parsed", ask, repay })
    }

    /// CBE messages whose online receipt hasn't been looked up yet:
    /// (message id, receipt id), at most `limit`.
    pub fn receipts_to_fetch(&self, limit: usize) -> Result<Vec<(String, String)>> {
        let mut st = self.conn.prepare("SELECT id, parsed FROM messages WHERE deleted = 0 ORDER BY received_at DESC")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (id, json) = row?;
            let Ok(p) = serde_json::from_str::<Parsed>(&json) else { continue };
            if let Some(receipt) = crate::receipt::wanted(&p) {
                out.push((id, receipt.to_string()));
                if out.len() >= limit {
                    break;
                }
            }
        }
        Ok(out)
    }

    /// Fill a message in from its receipt (`None`: the bank has no receipt
    /// for it; don't ask again). The change syncs like any other.
    pub fn apply_receipt(&self, message_id: &str, json: Option<&str>) -> Result<()> {
        let stored: String =
            self.conn.query_row("SELECT parsed FROM messages WHERE id = ?1", [message_id], |r| r.get(0))?;
        let mut p: Parsed = serde_json::from_str(&stored)?;
        match json {
            Some(j) => crate::receipt::apply(&mut p, j)?,
            None => p.receipt_checked = true,
        }
        self.conn.execute(
            "UPDATE messages SET parsed = ?2, updated_at = ?3 WHERE id = ?1",
            params![message_id, serde_json::to_string(&p)?, stamp()],
        )?;
        Ok(())
    }

    pub fn message_count(&self) -> Result<usize> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM messages WHERE deleted = 0", [], |r| r.get::<_, i64>(0))? as usize)
    }

    /// Messages that looked like money but matched no rule, newest first.
    pub fn unrecognized(&self) -> Result<Vec<Value>> {
        let mut st = self
            .conn
            .prepare("SELECT sender, body, received_at, reason FROM unrecognized ORDER BY received_at DESC LIMIT 200")?;
        let v = st
            .query_map([], |r| {
                Ok(json!({ "sender": r.get::<_, String>(0)?, "body": r.get::<_, String>(1)?,
                           "received_at": r.get::<_, i64>(2)?, "reason": r.get::<_, String>(3)? }))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    fn parsed_messages(&self) -> Result<Vec<Parsed>> {
        let mut st = self.conn.prepare("SELECT parsed FROM messages WHERE deleted = 0")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        // A row from a newer app version with fields this one doesn't know
        // still parses; one that doesn't parse at all is skipped.
        Ok(rows.iter().filter_map(|s| serde_json::from_str(s).ok()).collect())
    }

    // ── Transactions ────────────────────────────────────────────────────────

    /// Every transaction, oldest first, with its category worked out: set by
    /// hand, else learned for its recipient, else automatic (packages, fees),
    /// else by exact amount.
    pub fn transactions(&self) -> Result<Vec<TxnView>> {
        let core = self.core_settings()?;
        let mut txns = reconcile(&self.parsed_messages()?, &core.own_numbers);

        type Meta = (Option<String>, Option<String>, Vec<String>);
        let mut meta: HashMap<String, Meta> = HashMap::new();
        {
            let mut st = self.conn.prepare("SELECT id, category, note, settles FROM txn_meta WHERE deleted = 0")?;
            let rows = st.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get(1)?, r.get(2)?, r.get::<_, Option<String>>(3)?))
            })?;
            for row in rows {
                let (id, cat, note, settles) = row?;
                let settles = settles.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
                meta.insert(id, (cat, note, settles));
            }
        }
        let rules: HashMap<String, String> = {
            let mut st = self.conn.prepare("SELECT key, category FROM recipient_rules WHERE deleted = 0")?;
            let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
            v
        };

        let mut out = Vec::with_capacity(txns.len());
        for mut t in txns.drain(..) {
            let (manual, note, settles) = meta.remove(&t.id).unwrap_or_default();
            let learned = (t.flow == Flow::Spend).then(|| recipient_key(&t)).flatten().and_then(|k| rules.get(&k).cloned());
            let by_amount = (t.flow == Flow::Spend && t.kind == Kind::TransferOut)
                .then(|| core.amount_rules.get(&t.amount).cloned())
                .flatten();
            t.category = manual.or(learned).or(t.category.take()).or(by_amount);
            out.push(TxnView { txn: t, note, settles, repaid: 0, counted: 0 });
        }
        settle::apply(&mut out);
        Ok(out)
    }

    /// Set a transaction's category (None: back to automatic). With
    /// `remember`, every payment to the same recipient gets it unless set by
    /// hand. Returns how many other untagged transactions got tagged.
    pub fn categorize(&self, id: &str, category: Option<&str>, remember: bool) -> Result<usize> {
        let before = self.transactions()?;
        let untagged: HashSet<&str> =
            before.iter().filter(|t| t.txn.category.is_none() && t.txn.id != id).map(|t| t.txn.id.as_str()).collect();
        let key = before.iter().find(|t| t.txn.id == id).and_then(|t| recipient_key(&t.txn));
        let now = stamp();
        self.with_tx(|s| {
            s.conn.execute(
                "INSERT INTO txn_meta (id, category, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET category = excluded.category, updated_at = excluded.updated_at, deleted = 0",
                params![id, category, now],
            )?;
            if let (true, Some(key), Some(cat)) = (remember, &key, category) {
                s.conn.execute(
                    "INSERT INTO recipient_rules (key, category, updated_at) VALUES (?1, ?2, ?3)
                     ON CONFLICT(key) DO UPDATE SET category = excluded.category, updated_at = excluded.updated_at, deleted = 0",
                    params![key, cat, now],
                )?;
            }
            Ok(())
        })?;
        let after = self.transactions()?;
        Ok(after.iter().filter(|t| untagged.contains(t.txn.id.as_str()) && t.txn.category.is_some()).count())
    }

    /// Record that `repay_id` pays back `settles` (empty: it pays back nothing).
    pub fn link(&self, repay_id: &str, settles: &[String]) -> Result<()> {
        let value = (!settles.is_empty()).then(|| serde_json::to_string(settles)).transpose()?;
        self.conn.execute(
            "INSERT INTO txn_meta (id, settles, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET settles = excluded.settles, updated_at = excluded.updated_at, deleted = 0",
            params![repay_id, value, stamp()],
        )?;
        Ok(())
    }

    /// What a transaction might pay back, best first.
    pub fn suggest(&self, repay_id: &str) -> Result<Vec<Suggestion>> {
        let all = self.transactions()?;
        let Some(r) = all.iter().find(|t| t.txn.id == repay_id) else { return Ok(Vec::new()) };
        Ok(settle::suggest(&all, r))
    }

    pub fn set_note(&self, id: &str, note: &str) -> Result<()> {
        let note = Some(note.trim()).filter(|n| !n.is_empty());
        self.conn.execute(
            "INSERT INTO txn_meta (id, note, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET note = excluded.note, updated_at = excluded.updated_at, deleted = 0",
            params![id, note, stamp()],
        )?;
        Ok(())
    }

    // ── Settings ────────────────────────────────────────────────────────────

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1 AND deleted = 0", [key], |r| r.get(0))
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at, deleted = 0",
            params![key, value, stamp()],
        )?;
        Ok(())
    }

    fn json_setting(&self, key: &str, defaults: Value) -> Result<Value> {
        let stored = self.setting(key)?.and_then(|s| serde_json::from_str(&s).ok());
        Ok(with_defaults(stored, defaults))
    }

    /// The app's settings JSON (`own_numbers`, `amount_rules`, …).
    pub fn settings(&self) -> Result<Value> {
        self.json_setting("settings", default_settings())
    }

    pub fn save_settings(&self, v: &Value) -> Result<()> {
        self.set_setting("settings", &v.to_string())
    }

    /// The budget plan JSON (salary, bills, weekly limits, goal).
    pub fn plans(&self) -> Result<Value> {
        self.json_setting("plans", default_plans())
    }

    pub fn save_plans(&self, v: &Value) -> Result<()> {
        self.set_setting("plans", &v.to_string())
    }

    fn core_settings(&self) -> Result<CoreSettings> {
        let s = self.settings()?;
        let own_numbers = s["own_numbers"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).map(crate::parse::norm_phone).collect())
            .unwrap_or_default();
        let amount_rules = s["amount_rules"]
            .as_object()
            .map(|o| {
                o.iter()
                    .filter_map(|(k, v)| Some((k.parse::<i64>().ok()?, v.as_str()?.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        Ok(CoreSettings { own_numbers, amount_rules })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWN: &str = "0911223344";

    fn store() -> (Store, Parser) {
        let s = Store::open_in_memory().unwrap();
        s.save_settings(&json!({ "own_numbers": [OWN], "amount_rules": { "1500": "transport" } })).unwrap();
        (s, Parser::with_defaults().unwrap())
    }

    fn transfer(name: &str, amount: &str, minute: u32) -> String {
        format!(
            "Dear Abebe \nYou have transferred ETB {amount} to {name} (2519****8470) on 02/10/2026 19:{minute:02}:59. \
             Your transaction number is DJ21AAAA{minute:02}. The service fee is  ETB 0.87 and  15% VAT on the service fee is ETB 0.13. \
             Your current E-Money Account  balance is ETB 2,791.06."
        )
    }

    const T0: i64 = 1_791_000_000_000;

    #[test]
    fn ingest_asks_about_new_spending_once() {
        let (s, p) = store();
        let body = transfer("Kebede Alemu", "300.00", 1);
        let first = s.ingest(&p, "127", &body, T0).unwrap();
        assert_eq!(first.outcome, "parsed");
        assert_eq!(first.ask.unwrap().txn.amount, 30000);
        assert_eq!(s.ingest(&p, "127", &body, T0).unwrap().outcome, "duplicate");
        assert_eq!(s.message_count().unwrap(), 1);
    }

    #[test]
    fn amount_rule_tags_without_asking() {
        let (s, p) = store();
        let r = s.ingest(&p, "127", &transfer("Taxi Driver", "15.00", 2), T0).unwrap();
        assert!(r.ask.is_none());
        assert_eq!(s.transactions().unwrap()[0].txn.category.as_deref(), Some("transport"));
    }

    #[test]
    fn remembering_a_recipient_tags_the_others() {
        let (s, p) = store();
        for m in 1..=3 {
            s.ingest(&p, "127", &transfer("Kebede Alemu", "300.00", m), T0 + m as i64).unwrap();
        }
        let id = s.transactions().unwrap()[0].txn.id.clone();
        assert_eq!(s.categorize(&id, Some("family"), true).unwrap(), 2);
        assert!(s.transactions().unwrap().iter().all(|t| t.txn.category.as_deref() == Some("family")));
        // The next one is tagged on arrival, so nothing to ask.
        assert!(s.ingest(&p, "127", &transfer("Kebede Alemu", "300.00", 9), T0 + 9).unwrap().ask.is_none());
    }

    #[test]
    fn hand_set_category_beats_rules() {
        let (s, p) = store();
        s.ingest(&p, "127", &transfer("Friend", "15.00", 3), T0).unwrap();
        let id = s.transactions().unwrap()[0].txn.id.clone();
        s.categorize(&id, Some("friends"), false).unwrap();
        assert_eq!(s.transactions().unwrap()[0].txn.category.as_deref(), Some("friends"));
        s.categorize(&id, None, false).unwrap();
        assert_eq!(s.transactions().unwrap()[0].txn.category.as_deref(), Some("transport"));
    }

    #[test]
    fn unrecognized_keeps_text_locally() {
        let (s, p) = store();
        let r = s.ingest(&p, "127", "Dear Abebe You have withdrawn ETB 500.00 from agent 1 on 01/11/2026.", T0).unwrap();
        assert_eq!(r.outcome, "unrecognized");
        assert_eq!(s.unrecognized().unwrap().len(), 1);
        assert_eq!(s.message_count().unwrap(), 0);
    }

    #[test]
    fn a_new_rule_clears_the_not_understood_list() {
        let (s, p) = store();
        let body = "Dear Abebe You have withdrawn ETB 500.00 from agent 1 on 01/11/2026.";
        s.ingest(&p, "127", body, T0).unwrap();
        let mut rules = crate::RuleSet::from_toml(crate::parse::DEFAULT_RULES).unwrap();
        rules.rules.push(crate::Rule {
            id: "telebirr.withdraw".into(),
            source: "telebirr".into(),
            senders: vec!["127".into()],
            kind: Kind::Debit,
            priority: 0,
            pattern: "You have withdrawn ETB {amount}".into(),
            extract: vec![],
            date_format: None,
            counterparty: None,
            description: None,
        });
        let newer = Parser::new(rules).unwrap();
        assert_eq!(s.ingest(&newer, "127", body, T0).unwrap().outcome, "parsed");
        assert!(s.unrecognized().unwrap().is_empty());
    }

    #[test]
    fn settings_fill_in_defaults() {
        let s = Store::open_in_memory().unwrap();
        assert_eq!(s.plans().unwrap()["save_percent"], 20);
        s.save_plans(&json!({ "salary": 6000000 })).unwrap();
        let p = s.plans().unwrap();
        assert_eq!((p["salary"].as_i64(), p["save_percent"].as_i64()), (Some(6000000), Some(20)));
    }

    fn received(name: &str, amount: &str, minute: u32) -> String {
        format!(
            "Dear Abebe \nYou have received ETB {amount} from {name}(2519****8470)  on 02/10/2026 19:{minute:02}:30. \
             Your transaction number is DJIN{minute:06}. Your current E-Money Account balance is ETB 665.16."
        )
    }

    fn view<'a>(all: &'a [TxnView], amount: i64) -> &'a TxnView {
        all.iter().find(|t| t.txn.amount == amount).unwrap()
    }

    #[test]
    fn split_bill_counts_only_my_share() {
        let (s, p) = store();
        s.ingest(&p, "127", &transfer("Cafe Owner", "600.00", 1), T0).unwrap();
        let r1 = s.ingest(&p, "127", &received("Kebede Alemu", "200.00", 5), T0 + 1).unwrap();
        // A different person paying: not "likely" (no payment to them).
        assert!(r1.repay.is_none());
        let all = s.transactions().unwrap();
        let (pay, back) = (view(&all, 60000).txn.id.clone(), view(&all, 20000).txn.id.clone());
        s.link(&back, &[pay.clone()]).unwrap();
        let all = s.transactions().unwrap();
        assert_eq!((view(&all, 60000).counted, view(&all, 60000).repaid), (40000, 20000));
        assert_eq!(view(&all, 20000).counted, 0, "the share coming back isn't income");
        // Unlinking restores both.
        s.link(&back, &[]).unwrap();
        let all = s.transactions().unwrap();
        assert_eq!((view(&all, 60000).counted, view(&all, 20000).counted), (60000, 20000));
    }

    #[test]
    fn lent_is_not_spending_and_repayment_is_suggested() {
        let (s, p) = store();
        s.ingest(&p, "127", &transfer("Kebede Alemu", "500.00", 1), T0).unwrap();
        let pay = s.transactions().unwrap()[0].txn.id.clone();
        s.categorize(&pay, Some(settle::LENT), false).unwrap();
        assert_eq!(s.transactions().unwrap()[0].counted, 0);

        let r = s.ingest(&p, "127", &received("Kebede Alemu", "500.00", 9), T0 + 1).unwrap();
        let (back, sugg) = r.repay.expect("same person, same amount: likely");
        assert_eq!(sugg.txn.txn.id, pay);
        s.link(&back.txn.id, &[pay.clone()]).unwrap();
        let all = s.transactions().unwrap();
        assert_eq!(view(&all, 50000 + 0).repaid, 50000);
        assert!(all.iter().all(|t| t.counted == 0), "lent and repaid: nothing counts");
    }

    #[test]
    fn written_off_loan_becomes_spending() {
        let (s, p) = store();
        s.ingest(&p, "127", &transfer("Kebede Alemu", "500.00", 1), T0).unwrap();
        let pay = s.transactions().unwrap()[0].txn.id.clone();
        s.categorize(&pay, Some(settle::LENT), false).unwrap();
        s.categorize(&pay, Some("written_off"), false).unwrap();
        assert_eq!(s.transactions().unwrap()[0].counted, 50000);
    }

    #[test]
    fn borrowed_and_paid_back_cancel_out() {
        let (s, p) = store();
        s.ingest(&p, "127", &received("Kebede Alemu", "1,000.00", 1), T0).unwrap();
        let loan = s.transactions().unwrap()[0].txn.id.clone();
        s.categorize(&loan, Some(settle::BORROWED), false).unwrap();
        s.ingest(&p, "127", &transfer("Kebede Alemu", "1,000.00", 30), T0 + 1).unwrap();
        let all = s.transactions().unwrap();
        let back = all.iter().find(|t| t.txn.flow == Flow::Spend).unwrap().txn.id.clone();
        let sugg = s.suggest(&back).unwrap();
        assert_eq!(sugg[0].txn.txn.id, loan);
        s.link(&back, &[loan]).unwrap();
        assert!(s.transactions().unwrap().iter().all(|t| t.counted == 0));
    }

    #[test]
    fn old_database_gets_the_settles_column() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE txn_meta (id TEXT PRIMARY KEY, category TEXT, note TEXT, updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0)").unwrap();
        let s = Store::init(conn).unwrap();
        s.link("x", &["y".to_string()]).unwrap();
    }

    #[test]
    fn cbe_receipt_turns_a_debit_into_an_own_account_transfer() {
        let (s, p) = store();
        let debit = "Dear Abebe A debit transaction of ETB 500.00 has occurred on your account 1********4321. \
                     Your current balance is ETB 984.42. https://mbreciept.cbe.com.et/v2-aaaaaaaaaaaaaaaaaaaa";
        s.ingest(&p, "CBE", debit, T0).unwrap();
        assert_eq!(s.transactions().unwrap()[0].txn.flow, Flow::Spend);
        let todo = s.receipts_to_fetch(10).unwrap();
        assert_eq!(todo.len(), 1);
        assert_eq!(todo[0].1, "v2-aaaaaaaaaaaaaaaaaaaa");
        let receipt = format!(r#"{{"creditAccountHolder":"Tele Birr Settlement Account","description":"TeleB{OWN}"}}"#);
        s.apply_receipt(&todo[0].0, Some(&receipt)).unwrap();
        let t = &s.transactions().unwrap()[0].txn;
        assert_eq!((t.flow, t.kind), (Flow::Internal, Kind::TransferOut));
        assert!(s.receipts_to_fetch(10).unwrap().is_empty(), "fetched once");
    }

    #[test]
    fn notes() {
        let (s, p) = store();
        s.ingest(&p, "127", &transfer("Kebede Alemu", "300.00", 4), T0).unwrap();
        let id = s.transactions().unwrap()[0].txn.id.clone();
        s.set_note(&id, " lunch ").unwrap();
        assert_eq!(s.transactions().unwrap()[0].note.as_deref(), Some("lunch"));
    }
}
