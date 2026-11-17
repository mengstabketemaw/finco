//! Money that comes back: split bills, lending and borrowing.
//!
//! A repayment *settles* one or more earlier transactions:
//! - A friend's share of a bill you paid: the payment then counts only what
//!   is left (600 coffee − 2 × 200 back = 200 spent), and the money in isn't
//!   income.
//! - A payment tagged `lent` doesn't count as spending at all; it waits on
//!   the "owed to me" list until repayments cover it (or it's written off,
//!   which makes it ordinary spending again).
//! - Money in tagged `borrowed` isn't income; paying it back settles it.
//!
//! Fees are never settled: they were really paid.

use std::collections::HashMap;

use chrono::Duration;
use serde::{Deserialize, Serialize};

use crate::reconcile::{Flow, Txn};
use crate::store::TxnView;

pub const LENT: &str = "lent";
pub const BORROWED: &str = "borrowed";
/// A part of a payment that friends paid back outside the app (cash): it
/// isn't counted, like a repayment.
pub const CASH_BACK: &str = "cash_back";

/// A part of a payment with its own category.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Split {
    /// Santim.
    pub amount: i64,
    pub category: String,
}

/// Paid back in cash, per its split parts.
fn cash_back(t: &TxnView) -> i64 {
    t.splits.iter().filter(|p| p.category == CASH_BACK).map(|p| p.amount).sum()
}

/// How far back a repayment looks for what it might settle.
const LOOKBACK_DAYS: i64 = 60;
/// Friends often send their share first and you pay the bill after, so a
/// payment up to a week later can be settled too.
const AHEAD_DAYS: i64 = 7;

/// Fill in `repaid` and `counted` on every transaction.
pub fn apply(txns: &mut [TxnView]) {
    // A repayment's money is shared out over what it settles, in order,
    // each taking at most what is still open on it; earlier repayments first.
    let open: HashMap<&str, i64> = txns.iter().map(|t| (t.txn.id.as_str(), t.txn.amount - cash_back(t))).collect();
    let mut repayments: Vec<&TxnView> = txns.iter().filter(|t| !t.settles.is_empty()).collect();
    repayments.sort_by(|a, b| a.txn.occurred_at.cmp(&b.txn.occurred_at));
    let mut repaid: HashMap<String, i64> = HashMap::new();
    for r in repayments {
        let mut left = r.txn.amount;
        for id in &r.settles {
            let Some(&cap) = open.get(id.as_str()) else { continue };
            let got = repaid.entry(id.clone()).or_default();
            let give = left.min((cap - *got).max(0));
            *got += give;
            left -= give;
        }
    }
    for t in txns.iter_mut() {
        t.repaid = repaid.get(&t.txn.id).copied().unwrap_or(0);
        let cat = t.txn.category.as_deref();
        t.counted = if !t.settles.is_empty() || cat == Some(LENT) || cat == Some(BORROWED) {
            0
        } else if matches!(t.txn.flow, Flow::Spend | Flow::Income) {
            (t.txn.amount - t.repaid - cash_back(t)).max(0)
        } else {
            0
        };
    }
}

/// What's still open on a transaction others can settle.
pub fn remaining(t: &TxnView) -> i64 {
    (t.txn.amount - t.repaid - cash_back(t)).max(0)
}

/// An earlier transaction a repayment might settle.
#[derive(Debug, Clone, Serialize)]
pub struct Suggestion {
    pub txn: TxnView,
    /// Still open before this repayment.
    pub remaining: i64,
    pub score: i32,
    /// Strong enough to ask about in a notification.
    pub likely: bool,
}

fn same_person(a: &Txn, b: &Txn) -> bool {
    let name = |t: &Txn| t.counterparty.as_deref().map(|c| c.trim().to_lowercase()).filter(|c| !c.is_empty());
    let (na, nb) = (name(a), name(b));
    if na.is_some() && nb.is_some() {
        return na == nb;
    }
    // telebirr shows only the last four digits of a number, which two
    // people can share, so the account decides only when a name is missing.
    a.source == b.source && a.counterparty_account.is_some() && a.counterparty_account == b.counterparty_account
}

/// What `repay` (money in, or money out repaying a loan) might settle, best
/// first. Money in looks at your payments; money out at money you borrowed.
pub fn suggest(all: &[TxnView], repay: &TxnView) -> Vec<Suggestion> {
    let r = &repay.txn;
    let incoming = r.flow == Flow::Income;
    let mut out: Vec<Suggestion> = all
        .iter()
        .filter(|c| {
            let t = &c.txn;
            t.id != r.id
                && c.settles.is_empty()
                && !repay.settles.contains(&t.id)
                && r.occurred_at - t.occurred_at <= Duration::days(LOOKBACK_DAYS)
                && t.occurred_at - r.occurred_at <= Duration::days(AHEAD_DAYS)
                && remaining(c) > 0
                && if incoming {
                    t.flow == Flow::Spend && !matches!(t.kind, crate::Kind::Fee | crate::Kind::Package | crate::Kind::Airtime)
                } else {
                    t.flow == Flow::Income && t.category.as_deref() == Some(BORROWED)
                }
        })
        .map(|c| {
            let t = &c.txn;
            let open = remaining(c);
            let person = same_person(t, r);
            let marked = matches!(t.category.as_deref(), Some(LENT) | Some(BORROWED));
            let mut score = 0;
            if person {
                score += 4;
            }
            if marked {
                score += 2;
            }
            if open == r.amount {
                score += 3;
            } else if open > r.amount {
                score += 1;
            }
            if (r.occurred_at - t.occurred_at).num_days().abs() <= 3 {
                score += 1;
            }
            let likely = (person && open >= r.amount) || (marked && open == r.amount);
            Suggestion { txn: c.clone(), remaining: open, score, likely }
        })
        .collect();
    // Everything that could be settled, best guesses first: the app shows
    // the good ones (score 2+) and lets you search the rest.
    out.sort_by(|a, b| b.score.cmp(&a.score).then(b.txn.txn.occurred_at.cmp(&a.txn.txn.occurred_at)));
    out
}
