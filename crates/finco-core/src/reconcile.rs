//! Turns parsed messages into transactions and decides what each one means
//! for the budget: spending, income, money moved between your own accounts,
//! or a loan movement.
//!
//! - **Own-account transfers** are pairs: money leaves one account and the
//!   same amount reaches another within a few minutes, and one side names the
//!   other's bank (Zemen → telebirr, CBE → telebirr). An RTGS transfer pairs
//!   with a credit from the National Bank of Ethiopia within a day. Both legs become
//!   `internal`, so they count as neither spending nor income. A transfer to
//!   your own phone number counts too, even if its other leg is missing.
//! - **Loans** (telebirr Endekise and the like) never count as spending or
//!   income. The payment a loan covered has its own message and counts there.
//!   Fees on any transaction (`fee`) are a cost of their own.
//! - **Automatic categories**: packages and airtime for your own number are
//!   `internet`, for anyone else's `family`; standalone fees are `fees`.

use chrono::{DateTime, Duration, FixedOffset};
use serde::Serialize;

use crate::parse::{Kind, Parsed};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Flow {
    Spend,
    Income,
    Internal,
    Loan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Txn {
    /// `source:reference`, or `source:time:amount` when there's no reference.
    pub id: String,
    pub occurred_at: DateTime<FixedOffset>,
    pub source: String,
    pub kind: Kind,
    pub flow: Flow,
    pub amount: i64,
    pub fee: i64,
    pub balance: Option<i64>,
    pub outstanding: Option<i64>,
    pub counterparty: Option<String>,
    pub counterparty_account: Option<String>,
    /// Your account at `source`, masked.
    pub account: Option<String>,
    pub description: Option<String>,
    pub target_number: Option<String>,
    pub reference: Option<String>,
    pub category: Option<String>,
    /// The other leg of an own-account transfer, or the loan message this
    /// payout belongs to.
    pub pair_id: Option<String>,
}

/// How far apart the two legs of one transfer can be. Some banks' messages
/// have no time, so the receive time is used, which can lag a little.
const PAIR_WINDOW_MIN: i64 = 10;

/// RTGS (the National Bank's system for large transfers between banks) can
/// take hours to land.
const RTGS_WINDOW_HOURS: i64 = 24;

fn is_rtgs(t: &Txn) -> bool {
    t.description.as_deref().is_some_and(|d| d.contains("RTGS"))
}

/// An RTGS transfer arrives from the National Bank of Ethiopia, which settles
/// it, rather than from the sending bank.
fn is_nbe(counterparty: &Option<String>) -> bool {
    counterparty.as_deref().is_some_and(|c| c.to_lowercase().contains("national bank of ethiopia"))
}

fn window(out: &Txn) -> Duration {
    if is_rtgs(out) {
        Duration::hours(RTGS_WINDOW_HOURS)
    } else {
        Duration::minutes(PAIR_WINDOW_MIN)
    }
}

/// Names each source goes by in other banks' messages.
fn names(source: &str) -> &'static [&'static str] {
    match source {
        "telebirr" => &["telebirr"],
        "cbe" => &["commercial bank of ethiopia", "cbe"],
        "cbebirr" => &["cbe birr", "cbebirr"],
        "zemen" => &["zemen"],
        "yaya" => &["yaya"],
        "addis" => &["addis bank"],
        _ => &[],
    }
}

fn mentions(counterparty: &Option<String>, source: &str) -> bool {
    let Some(c) = counterparty else { return false };
    let c = c.to_lowercase();
    names(source).iter().any(|n| c.contains(n))
}

pub fn reconcile(parsed: &[Parsed], own_numbers: &[String]) -> Vec<Txn> {
    let mut txns: Vec<Txn> = parsed.iter().filter(|p| p.kind != Kind::Ignore).map(to_txn).collect();
    txns.sort_by_key(|t| t.occurred_at);
    merge_split_fees(&mut txns);

    let own = |n: &Option<String>| n.as_ref().is_some_and(|n| own_numbers.contains(n));

    // Own-account transfers.
    for i in 0..txns.len() {
        if !is_out(&txns[i]) || txns[i].pair_id.is_some() {
            continue;
        }
        let o = &txns[i];
        let partner = (0..txns.len())
            .filter(|&j| {
                let n = &txns[j];
                is_in(n)
                    && n.pair_id.is_none()
                    && n.source != o.source
                    && n.amount == o.amount
                    && (n.occurred_at - o.occurred_at).abs() <= window(o)
                    && (mentions(&n.counterparty, &o.source)
                        || mentions(&o.counterparty, &n.source)
                        || (is_rtgs(o) && is_nbe(&n.counterparty)))
                    // A transfer to a named account (someone at CBE, say) only
                    // counts if that account is the one receiving it.
                    && match (&o.counterparty_account, &n.account) {
                        (Some(a), Some(b)) => a == b,
                        (Some(_), None) => false,
                        (None, _) => true,
                    }
            })
            .min_by_key(|&j| (txns[j].occurred_at - o.occurred_at).abs());
        if let Some(j) = partner {
            let (a, b) = (txns[i].id.clone(), txns[j].id.clone());
            txns[i].flow = Flow::Internal;
            txns[i].pair_id = Some(b);
            txns[j].flow = Flow::Internal;
            txns[j].pair_id = Some(a);
        } else if own(&txns[i].target_number) && txns[i].kind == Kind::TransferOut {
            txns[i].flow = Flow::Internal;
        }
    }

    // After pairing, which relies on messages naming the bank.
    name_bank_accounts(&mut txns);

    // A loan payout lands as a plain credit next to the loan message.
    for i in 0..txns.len() {
        if txns[i].kind != Kind::LoanDraw {
            continue;
        }
        let payout = (0..txns.len()).find(|&j| {
            let (l, c) = (&txns[i], &txns[j]);
            c.kind == Kind::Credit
                && c.flow == Flow::Income
                && c.source == l.source
                && c.amount == l.amount
                && (c.occurred_at - l.occurred_at).abs() <= Duration::minutes(2)
        });
        if let Some(j) = payout {
            txns[j].flow = Flow::Loan;
            txns[j].pair_id = Some(txns[i].id.clone());
        }
    }

    for t in &mut txns {
        if t.category.is_some() {
            continue;
        }
        t.category = match t.kind {
            Kind::Package | Kind::Airtime if own(&t.target_number) => Some("internet".into()),
            // Bought for someone else's number. Only once your own numbers
            // are known, or everything would look like it's for someone else.
            Kind::Package | Kind::Airtime if !own_numbers.is_empty() && t.target_number.is_some() => {
                Some("family".into())
            }
            Kind::Fee => Some("fees".into()),
            _ => None,
        };
    }
    txns
}

/// Zemen charges a transfer's fee as a second debit with the same reference
/// (ETB 3000 to someone, then ETB 0.60). The smaller one is folded into the
/// larger as its fee.
fn merge_split_fees(txns: &mut Vec<Txn>) {
    let mut i = 0;
    while i < txns.len() {
        let twin = (0..txns.len()).find(|&j| {
            j != i
                && txns[j].reference.is_some()
                && txns[j].reference == txns[i].reference
                && txns[j].source == txns[i].source
                && txns[j].kind == txns[i].kind
                && txns[j].amount <= txns[i].amount
        });
        match twin {
            Some(j) => {
                let fee = txns[j].amount + txns[j].fee;
                txns[i].fee += fee;
                txns.remove(j);
                if j < i {
                    i -= 1;
                }
            }
            None => i += 1,
        }
    }
}

/// Which bank an account number belongs to, from how a message describes it.
fn bank_of(t: &Txn) -> Option<&'static str> {
    let named = |s: &Option<String>| s.as_deref().map(str::to_lowercase).unwrap_or_default();
    let (who, what) = (named(&t.counterparty), named(&t.description));
    if t.source == "cbe" || who.contains("commercial bank of ethiopia") || what.contains("commercial bank of ethiopia") {
        Some("CBE")
    } else {
        None
    }
}

/// A telebirr transfer to a bank says only "to Commercial Bank of Ethiopia
/// account number …0366". When the same account shows up elsewhere with a
/// name (money from it at CBE, a Zemen transfer to it), use that name, so
/// the transfer reads "Mahlet Kebede" rather than the bank's name.
fn name_bank_accounts(txns: &mut [Txn]) {
    use std::collections::HashMap;
    let mut names: HashMap<(&'static str, String), String> = HashMap::new();
    for t in txns.iter() {
        let (Some(bank), Some(acct), Some(name)) = (bank_of(t), &t.counterparty_account, &t.counterparty) else { continue };
        // People, not the bank itself or a company settlement account.
        let n = name.to_lowercase();
        if !n.contains("bank") && !n.contains("account") && n != "telebirr" {
            names.entry((bank, acct.clone())).or_insert_with(|| name.trim_start_matches("Miss ").trim_start_matches("Mr ").to_string());
        }
    }
    for t in txns.iter_mut() {
        let Some(bank) = bank_of(t) else { continue };
        let Some(acct) = t.counterparty_account.clone() else { continue };
        // Only a message that names just the bank ("Commercial Bank of
        // Ethiopia"); "Air Time Sales-mobile Banking" is a real recipient.
        let is_bank_name = t.counterparty.as_deref().is_some_and(|c| c.trim().eq_ignore_ascii_case("commercial bank of ethiopia"));
        if let (true, Some(name)) = (is_bank_name, names.get(&(bank, acct.clone()))) {
            t.counterparty = Some(name.clone());
            if t.description.is_none() {
                t.description = Some(format!("{bank} {acct}"));
            }
        }
    }
}

fn is_out(t: &Txn) -> bool {
    matches!(t.kind, Kind::TransferOut | Kind::Debit)
}

fn is_in(t: &Txn) -> bool {
    matches!(t.kind, Kind::TransferIn | Kind::Credit | Kind::Deposit)
}

/// The id a parsed message's transaction gets.
pub fn txn_id(p: &Parsed) -> String {
    match &p.reference {
        Some(r) => format!("{}:{r}", p.source),
        None => format!("{}:{}:{}", p.source, p.occurred_at.timestamp(), p.amount),
    }
}

fn to_txn(p: &Parsed) -> Txn {
    let flow = match p.kind.direction() {
        -1 => Flow::Spend,
        1 if p.kind == Kind::LoanDraw => Flow::Loan,
        1 => Flow::Income,
        _ => Flow::Loan,
    };
    let id = txn_id(p);
    Txn {
        id,
        occurred_at: p.occurred_at,
        source: p.source.clone(),
        kind: p.kind,
        flow,
        amount: p.amount,
        fee: p.fee,
        balance: p.balance,
        outstanding: p.outstanding,
        counterparty: p.counterparty.clone(),
        counterparty_account: p.counterparty_account.clone(),
        account: p.account.clone(),
        description: p.description.clone(),
        target_number: p.target_number.clone(),
        reference: p.reference.clone(),
        category: None,
        pair_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(source: &str, kind: Kind, amount: i64, at: &str) -> Parsed {
        Parsed {
            rule_id: String::new(),
            source: source.into(),
            kind,
            amount,
            fee: 0,
            balance: None,
            outstanding: None,
            counterparty: None,
            counterparty_account: None,
            account: None,
            reference: None,
            target_number: None,
            description: None,
            occurred_at: DateTime::parse_from_rfc3339(&format!("2026-10-23T{at}+03:00")).unwrap(),
            receipt_checked: false,
        }
    }

    fn own() -> Vec<String> {
        vec!["0911223344".into()]
    }

    #[test]
    fn zemen_to_own_telebirr_is_internal() {
        let mut out = p("zemen", Kind::TransferOut, 300000, "09:14:22");
        out.counterparty = Some("telebirr".into());
        out.target_number = Some("0911223344".into());
        let mut inn = p("telebirr", Kind::TransferIn, 300000, "09:14:45");
        inn.counterparty = Some("Zemen Bank".into());
        let t = reconcile(&[out, inn], &own());
        assert!(t.iter().all(|t| t.flow == Flow::Internal), "{t:?}");
        assert_eq!(t[0].pair_id.as_deref(), Some(t[1].id.as_str()));
    }

    #[test]
    fn transfer_to_own_number_without_other_leg_is_internal() {
        let mut out = p("zemen", Kind::TransferOut, 100000, "09:00:00");
        out.target_number = Some("0911223344".into());
        assert_eq!(reconcile(&[out], &own())[0].flow, Flow::Internal);
    }

    #[test]
    fn cbe_debit_into_telebirr_is_internal() {
        let out = p("cbe", Kind::Debit, 500000, "17:18:03");
        let mut inn = p("telebirr", Kind::TransferIn, 500000, "17:18:12");
        inn.counterparty = Some("Commercial Bank of Ethiopia".into());
        assert!(reconcile(&[out, inn], &own()).iter().all(|t| t.flow == Flow::Internal));
    }

    #[test]
    fn same_amount_to_a_person_is_not_internal() {
        let mut out = p("telebirr", Kind::TransferOut, 50000, "10:00:00");
        out.counterparty = Some("Kebede Alemu".into());
        let mut inn = p("cbe", Kind::TransferIn, 50000, "10:01:00");
        inn.counterparty = Some("Hana Bekele".into());
        let t = reconcile(&[out, inn], &own());
        assert_eq!((t[0].flow, t[1].flow), (Flow::Spend, Flow::Income));
    }

    #[test]
    fn transfer_to_someone_elses_account_at_the_same_bank_is_not_internal() {
        let mut out = p("telebirr", Kind::TransferOut, 170000, "09:19:22");
        out.counterparty = Some("Commercial Bank of Ethiopia".into());
        out.counterparty_account = Some("****0366".into());
        let mut inn = p("cbe", Kind::TransferIn, 170000, "09:20:00");
        inn.account = Some("****4321".into());
        let t = reconcile(&[out, inn], &own());
        assert_eq!(t[0].flow, Flow::Spend);
    }

    #[test]
    fn same_amount_to_a_named_account_is_not_the_wallet_top_up() {
        // Zemen sends 3000 to a person's CBE account and 3000 to the own
        // telebirr wallet at the same minute; only the second is internal.
        let mut person = p("zemen", Kind::TransferOut, 300000, "09:14:20");
        person.counterparty = Some("Miss Hana Bekele".into());
        person.counterparty_account = Some("****0366".into());
        person.reference = Some("108ETIP1".into());
        let mut wallet = p("zemen", Kind::TransferOut, 300000, "09:14:22");
        wallet.counterparty = Some("telebirr".into());
        wallet.target_number = Some("0911223344".into());
        wallet.reference = Some("131ETTB1".into());
        let mut inn = p("telebirr", Kind::TransferIn, 300000, "09:14:45");
        inn.counterparty = Some("Zemen Bank".into());
        let t = reconcile(&[person, wallet, inn], &own());
        assert_eq!(t[0].flow, Flow::Spend);
        assert_eq!((t[1].flow, t[2].flow), (Flow::Internal, Flow::Internal));
        assert_eq!(t[1].pair_id.as_deref(), Some(t[2].id.as_str()));
    }

    #[test]
    fn fee_sent_as_a_second_debit_is_merged() {
        let mut fee = p("zemen", Kind::TransferOut, 60, "09:14:27");
        fee.reference = Some("108ETIP1".into());
        let mut main = p("zemen", Kind::TransferOut, 300000, "09:14:30");
        main.reference = Some("108ETIP1".into());
        let t = reconcile(&[fee, main], &own());
        assert_eq!(t.len(), 1);
        assert_eq!((t[0].amount, t[0].fee), (300000, 60));
    }

    #[test]
    fn rtgs_out_pairs_with_a_national_bank_credit() {
        // Zemen's RTGS message leaves the receiving bank blank; the money
        // shows up at CBE "credited by NATIONAL BANK OF ETHIOPIA".
        let mut out = p("zemen", Kind::TransferOut, 3000000, "18:27:27");
        out.description = Some("RTGS transfer".into());
        let mut inn = p("cbe", Kind::TransferIn, 3000000, "21:29:06");
        inn.counterparty = Some("NATIONAL BANK OF ETHIOPIA".into());
        let t = reconcile(&[out, inn], &own());
        assert!(t.iter().all(|t| t.flow == Flow::Internal), "{t:?}");
    }

    #[test]
    fn national_bank_credit_without_rtgs_stays_income() {
        let out = p("zemen", Kind::TransferOut, 3000000, "18:27:27");
        let mut inn = p("cbe", Kind::TransferIn, 3000000, "18:29:06");
        inn.counterparty = Some("NATIONAL BANK OF ETHIOPIA".into());
        assert_eq!(reconcile(&[out, inn], &own())[1].flow, Flow::Income);
    }

    #[test]
    fn bank_account_gets_the_name_seen_elsewhere() {
        // Money from Mahlet's CBE account names it; a telebirr transfer to
        // the same account then reads as a transfer to her.
        let mut from_her = p("cbe", Kind::TransferIn, 300000, "09:00:00");
        from_her.counterparty = Some("Mahlet Kebede Tesfaye".into());
        from_her.counterparty_account = Some("****0366".into());
        let mut to_her = p("telebirr", Kind::TransferOut, 50000, "09:22:30");
        to_her.counterparty = Some("Commercial Bank of Ethiopia".into());
        to_her.counterparty_account = Some("****0366".into());
        let mut someone_else = p("telebirr", Kind::TransferOut, 7000, "07:17:00");
        someone_else.counterparty = Some("Commercial Bank of Ethiopia".into());
        someone_else.counterparty_account = Some("****9999".into());
        // A merchant whose name merely contains "Banking", sharing the last
        // four digits with a settlement account, keeps its own name.
        let mut merchant = p("cbe", Kind::TransferOut, 3000, "08:24:00");
        merchant.counterparty = Some("Air Time Sales-mobile Banking".into());
        merchant.counterparty_account = Some("****0162".into());
        let mut settlement = p("cbe", Kind::TransferOut, 3000, "08:30:00");
        settlement.counterparty = Some("Yaya-wallet Settlement Account".into());
        settlement.counterparty_account = Some("****0162".into());
        let t = reconcile(&[from_her, to_her, someone_else, merchant, settlement], &own());
        let by = |amount: i64| t.iter().find(|x| x.amount == amount && x.source != "cbe").unwrap();
        assert_eq!(by(50000).counterparty.as_deref(), Some("Mahlet Kebede Tesfaye"));
        assert_eq!(by(50000).description.as_deref(), Some("CBE ****0366"));
        assert_eq!(by(7000).counterparty.as_deref(), Some("Commercial Bank of Ethiopia"), "unknown account keeps the bank");
        let merchant = t.iter().find(|x| x.occurred_at.to_rfc3339().contains("08:24")).unwrap();
        assert_eq!(merchant.counterparty.as_deref(), Some("Air Time Sales-mobile Banking"));
    }

    #[test]
    fn legs_too_far_apart_are_not_paired() {
        let out = p("cbe", Kind::Debit, 500000, "10:00:00");
        let mut inn = p("telebirr", Kind::TransferIn, 500000, "10:30:00");
        inn.counterparty = Some("Commercial Bank of Ethiopia".into());
        assert_eq!(reconcile(&[out, inn], &own())[0].flow, Flow::Spend);
    }

    #[test]
    fn loans_are_neither_spend_nor_income() {
        let draw = p("telebirr", Kind::LoanDraw, 50000, "08:00:12");
        let payout = p("telebirr", Kind::Credit, 50000, "08:00:00");
        let used = p("telebirr", Kind::LoanUse, 5448, "13:47:01");
        let repaid = p("telebirr", Kind::LoanRepay, 5584, "17:18:12");
        let t = reconcile(&[draw, payout, used, repaid], &own());
        assert!(t.iter().all(|t| t.flow == Flow::Loan), "{t:?}");
    }

    #[test]
    fn packages_are_internet_for_own_number_and_family_for_others() {
        let mut mine = p("telebirr", Kind::Package, 7000, "18:12:05");
        mine.target_number = Some("0911223344".into());
        let mut gift = p("telebirr", Kind::Package, 9500, "21:04:45");
        gift.target_number = Some("0990000001".into());
        let t = reconcile(&[mine.clone(), gift.clone()], &own());
        assert_eq!(t[0].category.as_deref(), Some("internet"));
        assert_eq!(t[1].category.as_deref(), Some("family"));

        // Without known own numbers nothing is guessed.
        let t = reconcile(&[mine, gift], &[]);
        assert!(t.iter().all(|t| t.category.is_none()));
    }
}
