//! Bank and wallet SMS parsing.
//!
//! The formats live in data, not code: a [`RuleSet`] (bundled defaults in
//! `rules/default.toml`, later overridden by rules synced from Supabase) holds
//! one [`Rule`] per sender and message type. A rule has a main regex, which
//! must match for the rule to apply, and optional `extract` regexes that pick
//! up extra fields (fees, balance, reference) wherever they appear, so a bank
//! that reorders its sentences still parses.
//!
//! Before matching, every run of whitespace in the message becomes a single
//! space, so rules never deal with double spaces or line breaks.
//!
//! In patterns, `{amount}`, `{fee}`, `{vat}`, `{levy}`, `{balance}` and
//! `{outstanding}` stand for a named money group (`1,234.56`). Other fields
//! use ordinary named groups: `counterparty`, `counterparty_account`,
//! `account`, `reference`, `target_number`, `description`, `date`.
//!
//! Each SMS is parsed on its own. Pairing related messages (both legs of a
//! transfer between your own accounts, a loan top-up and the payment it
//! covered) is a later step.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, TimeZone};
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

/// The rules shipped with the app.
pub const DEFAULT_RULES: &str = include_str!("../rules/default.toml");

/// Fields written as `{name}` in a pattern and parsed as money.
const MONEY_FIELDS: &[&str] = &["amount", "fee", "vat", "levy", "balance", "outstanding"];
const MONEY_RE: &str = r"\d[\d,]*(?:\.\d+)?";

/// What a message records. Ethiopian banks don't say "salary" or "rent",
/// so this is about how money moved, not why; categories come later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Paid a merchant or biller.
    Payment,
    /// Bought a mobile data, voice or SMS package.
    Package,
    /// Bought airtime.
    Airtime,
    /// Sent money to a person or another account.
    TransferOut,
    /// Received money from a person or another account.
    TransferIn,
    /// Cash or branch deposit.
    Deposit,
    /// Money out with no detail about where it went.
    Debit,
    /// Money in with no detail about where it came from.
    Credit,
    /// A charge on its own (not the fee attached to another transaction).
    Fee,
    /// A loan was paid out to the account.
    LoanDraw,
    /// An overdraft-style credit line covered part of a payment. The payment
    /// itself arrives as its own message; this only raises what you owe.
    LoanUse,
    /// A loan repayment.
    LoanRepay,
    /// Not a transaction (OTP, wrong PIN, duplicate notice). Rules with this
    /// kind keep such messages out of the "unrecognised" list.
    Ignore,
}

impl Kind {
    /// -1 money leaves your accounts, +1 it arrives, 0 it only changes debt.
    pub fn direction(self) -> i8 {
        use Kind::*;
        match self {
            Payment | Package | Airtime | TransferOut | Debit | Fee => -1,
            TransferIn | Deposit | Credit | LoanDraw => 1,
            LoanUse | LoanRepay | Ignore => 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSet {
    #[serde(rename = "rule")]
    pub rules: Vec<Rule>,
}

impl RuleSet {
    pub fn from_toml(text: &str) -> Result<Self> {
        Ok(toml::from_str(text)?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Stable id, e.g. `telebirr.transfer_out.person`.
    pub id: String,
    /// The account the message is about: `telebirr`, `cbe`, `zemen`, …
    pub source: String,
    /// SMS sender ids, compared case-insensitively after trimming.
    pub senders: Vec<String>,
    pub kind: Kind,
    /// Higher runs first. Needed where one pattern is a prefix of another.
    #[serde(default)]
    pub priority: i32,
    pub pattern: String,
    #[serde(default)]
    pub extract: Vec<String>,
    /// chrono format for the `date` group. Formats without `%H` are date-only.
    #[serde(default)]
    pub date_format: Option<String>,
    /// Fixed values for messages that don't name them.
    #[serde(default)]
    pub counterparty: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// An incoming SMS.
#[derive(Debug, Clone)]
pub struct Sms<'a> {
    pub sender: &'a str,
    pub body: &'a str,
    /// When the phone received it.
    pub received_at: DateTime<FixedOffset>,
}

/// Amounts are in santim (1/100 birr).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parsed {
    pub rule_id: String,
    pub source: String,
    pub kind: Kind,
    pub amount: i64,
    /// Service fee + VAT + levies charged on top of `amount`.
    pub fee: i64,
    pub balance: Option<i64>,
    /// Loan balance after this message.
    pub outstanding: Option<i64>,
    pub counterparty: Option<String>,
    /// Masked to the last four digits.
    pub counterparty_account: Option<String>,
    /// Your account, masked to the last four digits.
    pub account: Option<String>,
    pub reference: Option<String>,
    /// Phone number a package or airtime was bought for, as 09XXXXXXXX.
    pub target_number: Option<String>,
    pub description: Option<String>,
    /// From the message when it has a date and time, otherwise when the
    /// phone received it.
    pub occurred_at: DateTime<FixedOffset>,
    /// The bank's online receipt was looked up (CBE), filling in what the
    /// SMS left out. See `receipt`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub receipt_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    Parsed(Parsed),
    /// Matched an ignore rule, or a known sender's message with no money in
    /// it (promotions, security warnings).
    Ignored { rule_id: Option<String> },
    /// A known sender's message that mentions money but matches no rule:
    /// probably a new or changed format. Keep it for review.
    Unrecognized { reason: String },
    /// No rules for this sender.
    UnknownSender,
}

struct Compiled {
    rule: Rule,
    senders: Vec<String>,
    main: Regex,
    extract: Vec<Regex>,
}

pub struct Parser {
    rules: Vec<Compiled>,
    /// Time zone of dates written in messages.
    tz: FixedOffset,
}

impl Parser {
    pub fn new(set: RuleSet) -> Result<Self> {
        let mut rules = set
            .rules
            .into_iter()
            .map(|rule| {
                let main = compile(&rule.pattern).with_context(|| format!("rule {}", rule.id))?;
                let extract = rule
                    .extract
                    .iter()
                    .map(|p| compile(p))
                    .collect::<Result<_>>()
                    .with_context(|| format!("rule {}", rule.id))?;
                if main.capture_names().flatten().all(|n| n != "amount") && rule.kind != Kind::Ignore {
                    bail!("rule {}: pattern has no {{amount}}", rule.id);
                }
                let senders = rule.senders.iter().map(|s| norm_sender(s)).collect();
                Ok(Compiled { rule, senders, main, extract })
            })
            .collect::<Result<Vec<_>>>()?;
        rules.sort_by_key(|c| -c.rule.priority);
        Ok(Self { rules, tz: FixedOffset::east_opt(3 * 3600).unwrap() })
    }

    /// The bundled rules.
    pub fn with_defaults() -> Result<Self> {
        Self::new(RuleSet::from_toml(DEFAULT_RULES)?)
    }

    pub fn parse(&self, sms: &Sms) -> Outcome {
        let sender = norm_sender(sms.sender);
        let text = normalize_ws(sms.body);
        let mut known = false;
        for c in self.rules.iter().filter(|c| c.senders.contains(&sender)) {
            known = true;
            let Some(caps) = c.main.captures(&text) else { continue };
            if c.rule.kind == Kind::Ignore {
                return Outcome::Ignored { rule_id: Some(c.rule.id.clone()) };
            }
            return match self.build(c, &caps, &text, sms) {
                Ok(p) => Outcome::Parsed(p),
                Err(e) => Outcome::Unrecognized { reason: format!("rule {}: {e:#}", c.rule.id) },
            };
        }
        if !known {
            Outcome::UnknownSender
        } else if mentions_money(&text) {
            Outcome::Unrecognized { reason: "no rule matched".into() }
        } else {
            Outcome::Ignored { rule_id: None }
        }
    }

    fn build(&self, c: &Compiled, main: &Captures, text: &str, sms: &Sms) -> Result<Parsed> {
        let extra: Vec<Captures> = c.extract.iter().filter_map(|r| r.captures(text)).collect();
        // The main pattern wins; then the extract patterns in order.
        let get = |name: &str| -> Option<String> {
            std::iter::once(main)
                .chain(extra.iter())
                .find_map(|caps| caps.name(name))
                .map(|m| m.as_str().trim().to_string())
                .filter(|s| !s.is_empty())
        };
        let money = |name: &str| -> Result<Option<i64>> {
            get(name).map(|s| parse_money(&s).with_context(|| format!("{name} {s:?}"))).transpose()
        };

        let amount = money("amount")?.context("no amount")?;
        let mut fee = 0;
        for part in ["fee", "vat", "levy"] {
            fee += money(part)?.unwrap_or(0);
        }
        let occurred_at = match (get("date"), &c.rule.date_format) {
            (Some(d), Some(fmt)) => self.parse_date(&d, fmt, sms.received_at)?,
            (Some(_), None) => bail!("date captured but rule has no date_format"),
            (None, _) => sms.received_at,
        };

        Ok(Parsed {
            rule_id: c.rule.id.clone(),
            source: c.rule.source.clone(),
            kind: c.rule.kind,
            amount,
            fee,
            balance: money("balance")?,
            outstanding: money("outstanding")?,
            counterparty: get("counterparty").or_else(|| c.rule.counterparty.clone()),
            counterparty_account: get("counterparty_account").map(|a| mask_account(&a)),
            account: get("account").map(|a| mask_account(&a)),
            reference: get("reference"),
            target_number: get("target_number").map(|n| norm_phone(&n)),
            description: get("description").or_else(|| c.rule.description.clone()),
            occurred_at,
            receipt_checked: false,
        })
    }

    fn parse_date(&self, s: &str, fmt: &str, received: DateTime<FixedOffset>) -> Result<DateTime<FixedOffset>> {
        if fmt.contains("%H") {
            let naive = NaiveDateTime::parse_from_str(s, fmt).with_context(|| format!("date {s:?} as {fmt}"))?;
            return Ok(self.tz.from_local_datetime(&naive).unwrap());
        }
        let day = NaiveDate::parse_from_str(s, fmt).with_context(|| format!("date {s:?} as {fmt}"))?;
        // A date with no time: the receive time is more precise. Banks date
        // some transfers with the next business day (Zemen sends "on
        // 2-Nov" on 1 Nov), so a date after the receive day is also taken
        // as "now". Only a message that arrived after its date (a late
        // delivery) keeps the date from its text.
        if day >= received.with_timezone(&self.tz).date_naive() {
            Ok(received)
        } else {
            Ok(self.tz.from_local_datetime(&day.and_hms_opt(0, 0, 0).unwrap()).unwrap())
        }
    }
}

fn compile(pattern: &str) -> Result<Regex> {
    let mut p = pattern.to_string();
    for f in MONEY_FIELDS {
        p = p.replace(&format!("{{{f}}}"), &format!("(?P<{f}>{MONEY_RE})"));
    }
    Ok(Regex::new(&p)?)
}

fn norm_sender(s: &str) -> String {
    s.trim().to_lowercase()
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn mentions_money(text: &str) -> bool {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(?:ETB|birr) ?\d|\d ?(?:Br\.|birr)").unwrap()).is_match(text)
}

/// `1,234.5` → 123450 santim.
pub fn parse_money(s: &str) -> Result<i64> {
    let s = s.replace(',', "");
    let (whole, frac) = s.split_once('.').unwrap_or((&s, ""));
    if frac.len() > 2 {
        bail!("more than two decimals");
    }
    let whole: i64 = whole.parse()?;
    let frac: i64 = if frac.is_empty() { 0 } else { format!("{frac:0<2}").parse()? };
    Ok(whole * 100 + frac)
}

/// Account numbers keep only their last four digits: `1000066220366`,
/// `1********0366` and `2519****0366` all become `****0366`. Short merchant
/// codes and ids without trailing digits are kept as they are.
pub fn mask_account(s: &str) -> String {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let masked = compact.contains('*') || compact.contains('x');
    let digits = compact.chars().filter(char::is_ascii_digit).count();
    if !(masked || digits >= 8) {
        return compact;
    }
    let tail: String = compact.chars().rev().take_while(char::is_ascii_digit).collect();
    if tail.len() < 4 {
        return compact;
    }
    let last4: String = tail.chars().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("****{last4}")
}

/// `251911223344`, `911223344` and `0911223344` all become `0911223344`.
pub fn norm_phone(s: &str) -> String {
    let d: String = s.chars().filter(char::is_ascii_digit).collect();
    match d.len() {
        12 if d.starts_with("251") => format!("0{}", &d[3..]),
        9 => format!("0{d}"),
        _ => d,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money() {
        assert_eq!(parse_money("1,234.56").unwrap(), 123456);
        assert_eq!(parse_money("100.0").unwrap(), 10000);
        assert_eq!(parse_money("3000").unwrap(), 300000);
        assert_eq!(parse_money("0.6").unwrap(), 60);
        assert!(parse_money("1.234").is_err());
    }

    #[test]
    fn masking() {
        assert_eq!(mask_account("1000066220366"), "****0366");
        assert_eq!(mask_account("1********0366"), "****0366");
        assert_eq!(mask_account("2519****1731"), "****1731");
        assert_eq!(mask_account("131xxxxxxxx5678"), "****5678");
        assert_eq!(mask_account("**** **** **** 8001"), "****8001");
        assert_eq!(mask_account("513076"), "513076");
        assert_eq!(mask_account("ab********cd"), "ab********cd");
    }

    #[test]
    fn phones() {
        assert_eq!(norm_phone("251911223344"), "0911223344");
        assert_eq!(norm_phone("911223344"), "0911223344");
        assert_eq!(norm_phone("0911223344"), "0911223344");
    }

    #[test]
    fn default_rules_compile() {
        Parser::with_defaults().unwrap();
    }
}
