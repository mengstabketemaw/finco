//! Runs the bundled rules over a JSON export of real SMS and reports how many
//! were parsed, ignored or not recognised. Prints only rule ids and counts,
//! plus the text of unrecognised messages (so check before sharing output).
//!
//!     cargo run -p finco-core --example parse_dump -- export.sms.json
//!
//! The export is a JSON array of {"sender", "body", "date"} where date is
//! milliseconds since the epoch (what KDE Connect reports).

use std::collections::BTreeMap;

use chrono::{FixedOffset, TimeZone};
use finco_core::{Outcome, Parser, Sms};
use serde::Deserialize;

#[derive(Deserialize)]
struct Msg {
    sender: String,
    body: String,
    date: i64,
}

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("usage: parse_dump <export.json>");
    let msgs: Vec<Msg> = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let parser = Parser::with_defaults()?;
    let tz = FixedOffset::east_opt(3 * 3600).unwrap();

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut unrecognized = Vec::new();
    for m in &msgs {
        let sms = Sms { sender: &m.sender, body: &m.body, received_at: tz.timestamp_millis_opt(m.date).unwrap() };
        let key = match parser.parse(&sms) {
            Outcome::Parsed(p) => format!("parsed    {}", p.rule_id),
            Outcome::Ignored { rule_id } => format!("ignored   {}", rule_id.as_deref().unwrap_or("(no money mentioned)")),
            Outcome::Unrecognized { reason } => {
                unrecognized.push((m.sender.clone(), reason, m.body.clone()));
                "UNRECOGNIZED".into()
            }
            Outcome::UnknownSender => format!("unknown   sender {}", m.sender.trim()),
        };
        *counts.entry(key).or_default() += 1;
    }
    for (k, n) in &counts {
        println!("{n:5}  {k}");
    }
    println!("{:5}  total", msgs.len());
    for (sender, reason, body) in unrecognized {
        println!("\n[{sender}] {reason}\n{body}");
    }
    Ok(())
}
