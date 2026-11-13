//! Shared finance-tracker logic used by the Android receiver (through JNI)
//! and the desktop app.

pub mod parse;
pub mod paths;
pub mod receipt;
pub mod reconcile;
pub mod settle;
pub mod store;
pub mod sync;

pub use parse::{Kind, Outcome, Parsed, Parser, Rule, RuleSet, Sms};
pub use reconcile::{reconcile, Flow, Txn};
pub use store::{Ingested, Store, TxnView};
