//! Parses and reconciles a JSON export of real SMS and prints the
//! transactions as JSON, for previewing the UI in a browser before the app
//! has its own database.
//!
//!     FINCO_OWN_NUMBERS=09XXXXXXXX cargo run -p finco-core --example export -- \
//!         export.sms.json > app/public/dev-data.json
//!
//! The output holds real amounts and names: keep it out of git (it is ignored).

use chrono::{FixedOffset, TimeZone};
use finco_core::{reconcile, Outcome, Parser, Sms};
use serde::Deserialize;

#[derive(Deserialize)]
struct Msg {
    sender: String,
    body: String,
    date: i64,
}

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("usage: export <export.json>");
    let msgs: Vec<Msg> = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let own: Vec<String> = std::env::var("FINCO_OWN_NUMBERS")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(finco_core::parse::norm_phone)
        .collect();
    let parser = Parser::with_defaults()?;
    let tz = FixedOffset::east_opt(3 * 3600).unwrap();
    let parsed: Vec<_> = msgs
        .iter()
        .filter_map(|m| {
            let sms = Sms { sender: &m.sender, body: &m.body, received_at: tz.timestamp_millis_opt(m.date).unwrap() };
            match parser.parse(&sms) {
                Outcome::Parsed(p) => Some(p),
                _ => None,
            }
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&reconcile(&parsed, &own))?);
    Ok(())
}
