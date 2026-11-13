//! Runs every case in tests/fixtures/sms.toml through the bundled rules and
//! compares the whole result, so a field that shouldn't be there fails too.

use chrono::DateTime;
use finco_core::parse::parse_money;
use finco_core::{Kind, Outcome, Parsed, Parser, Sms};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixtures {
    case: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    sender: String,
    received_at: String,
    body: String,
    expect: Expect,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expect {
    outcome: String,
    rule_id: Option<String>,
    kind: Option<Kind>,
    amount: Option<String>,
    fee: Option<String>,
    balance: Option<String>,
    outstanding: Option<String>,
    counterparty: Option<String>,
    counterparty_account: Option<String>,
    account: Option<String>,
    reference: Option<String>,
    target_number: Option<String>,
    description: Option<String>,
    occurred_at: Option<String>,
}

fn money(s: &Option<String>) -> Option<i64> {
    s.as_deref().map(|s| parse_money(s).unwrap())
}

fn expected(case: &Case) -> Outcome {
    let e = &case.expect;
    match e.outcome.as_str() {
        "ignored" => Outcome::Ignored { rule_id: e.rule_id.clone() },
        "unknown_sender" => Outcome::UnknownSender,
        "unrecognized" => Outcome::Unrecognized { reason: "no rule matched".into() },
        "parsed" => {
            let rule_id = e.rule_id.clone().expect("parsed case needs rule_id");
            Outcome::Parsed(Parsed {
                source: rule_id.split('.').next().unwrap().to_string(),
                rule_id,
                kind: e.kind.expect("parsed case needs kind"),
                amount: money(&e.amount).expect("parsed case needs amount"),
                fee: money(&e.fee).unwrap_or(0),
                balance: money(&e.balance),
                outstanding: money(&e.outstanding),
                counterparty: e.counterparty.clone(),
                counterparty_account: e.counterparty_account.clone(),
                account: e.account.clone(),
                reference: e.reference.clone(),
                target_number: e.target_number.clone(),
                description: e.description.clone(),
                occurred_at: DateTime::parse_from_rfc3339(e.occurred_at.as_ref().unwrap_or(&case.received_at))
                    .unwrap(),
                receipt_checked: false,
            })
        }
        other => panic!("{}: unknown outcome {other}", case.name),
    }
}

#[test]
fn fixtures() {
    let fixtures: Fixtures = toml::from_str(include_str!("fixtures/sms.toml")).unwrap();
    let parser = Parser::with_defaults().unwrap();
    let mut failures = Vec::new();
    for case in &fixtures.case {
        let sms = Sms {
            sender: &case.sender,
            body: &case.body,
            received_at: DateTime::parse_from_rfc3339(&case.received_at).unwrap(),
        };
        let got = parser.parse(&sms);
        let want = expected(case);
        if got != want {
            failures.push(format!("── {}\n   want {want:?}\n   got  {got:?}", case.name));
        }
    }
    assert!(failures.is_empty(), "{} of {} cases failed:\n{}", failures.len(), fixtures.case.len(), failures.join("\n"));
    println!("{} cases passed", fixtures.case.len());
}

/// Every rule is exercised by at least one fixture, so a rule can't rot
/// unnoticed.
#[test]
fn every_rule_has_a_fixture() {
    let text = include_str!("fixtures/sms.toml");
    let rules = finco_core::RuleSet::from_toml(finco_core::parse::DEFAULT_RULES).unwrap();
    let missing: Vec<_> =
        rules.rules.iter().map(|r| &r.id).filter(|id| !text.contains(&format!("rule_id = \"{id}\""))).collect();
    assert!(missing.is_empty(), "rules without a fixture: {missing:?}");
}
