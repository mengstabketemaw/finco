//! Prints the transactions in a finco database (e.g. one copied off the
//! phone with `adb exec-out run-as dev.finco.app cat files/finco.db`).
//!
//!     cargo run -p finco-core --example dump_db -- finco.db 2026-10-22

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: dump_db <finco.db> [date prefix]");
    let prefix = args.next().unwrap_or_default();
    let store = finco_core::Store::open(std::path::Path::new(&path))?;
    for t in store.transactions()? {
        let t = &t.txn;
        let at = t.occurred_at.to_rfc3339();
        if !at.starts_with(&prefix) {
            continue;
        }
        println!(
            "{} {:<9} {:?}/{:?} {:>10.2}  {}  {}",
            &at[..16],
            t.source,
            t.flow,
            t.kind,
            t.amount as f64 / 100.0,
            t.counterparty.as_deref().unwrap_or("-"),
            t.description.as_deref().unwrap_or(""),
        );
    }
    Ok(())
}
