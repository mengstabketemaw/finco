//! CBE mobile-banking receipts.
//!
//! A CBE debit SMS says only "a debit of ETB 500 has occurred" and links a
//! receipt (`https://mbreciept.cbe.com.et/<id>`). That receipt page loads
//! its details as JSON from
//! `https://Mb.cbe.com.et/api/v1/transactions/public/transaction-detail/<id>`,
//! which says who received the money:
//!
//! ```json
//! { "creditAccountHolder": "Tele Birr Settlement Account",
//!   "creditAccountNo": "E***********0162",
//!   "description": "TeleB0911223344",
//!   "platformTransactionType": "WALLET_TRANSFER", … }
//! ```
//!
//! [`apply`] fills in what the SMS left out (it never overwrites what the SMS
//! said), so a payment to your own telebirr counts as a transfer between your
//! accounts even when telebirr's own message is missing. The phone fetches
//! each receipt once (the HTTP side lives in `finco-sync`); older branch
//! receipts are PDFs and are skipped.

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::parse::{mask_account, norm_phone, Kind, Parsed};

/// The receipt-page API and the fixed headers the page itself sends.
pub const API: &str = "https://Mb.cbe.com.et/api/v1/transactions/public/transaction-detail/";
pub const HEADERS: &[(&str, &str)] = &[
    ("X-App-ID", "d1292e42-7400-49de-a2d3-9731caa4c819"),
    ("X-App-Version", "0a01980b-9859-1369-8198-59f403820000"),
];

/// Whether this message has an online receipt still to look up.
pub fn wanted(p: &Parsed) -> Option<&str> {
    if p.source != "cbe" || p.receipt_checked {
        return None;
    }
    // Mobile-banking receipts; branch receipts ("FT…") are PDFs.
    p.reference.as_deref().filter(|r| r.starts_with("v2-"))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Detail {
    credit_account_holder: Option<String>,
    credit_account_no: Option<String>,
    description: Option<String>,
    platform_transaction_type: Option<String>,
}

/// Fill in `p` from a receipt's JSON and mark it checked.
pub fn apply(p: &mut Parsed, json: &str) -> Result<()> {
    let d: Detail = serde_json::from_str(json).context("reading CBE receipt")?;
    let holder = d.credit_account_holder.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let text = d.description.as_deref().unwrap_or("");
    // "TeleB0911223344": the telebirr wallet the money went to.
    let wallet = text
        .strip_prefix("TeleB")
        .filter(|n| n.len() >= 9 && n.chars().all(|c| c.is_ascii_digit()))
        .map(norm_phone);
    let to_telebirr = wallet.is_some() || holder.is_some_and(|h| h.to_lowercase().contains("tele birr"));

    if p.counterparty.is_none() {
        p.counterparty = if to_telebirr { Some("telebirr".into()) } else { holder.map(str::to_string) };
    }
    if p.counterparty_account.is_none() && !to_telebirr {
        p.counterparty_account = d.credit_account_no.as_deref().map(mask_account);
    }
    if p.target_number.is_none() {
        p.target_number = wallet.clone();
    }
    if p.description.is_none() {
        p.description = d.platform_transaction_type.filter(|s| !s.is_empty()).map(|s| s.replace('_', " ").to_lowercase());
    }
    // Knowing where it went, a plain debit is a transfer.
    if p.kind == Kind::Debit && (holder.is_some() || wallet.is_some()) {
        p.kind = Kind::TransferOut;
    }
    p.receipt_checked = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Outcome, Parser, Sms};
    use chrono::DateTime;

    fn cbe_debit() -> Parsed {
        let body = "Dear Abebe Kebede Tesfaye A debit transaction of ETB 500.00 has occurred on your account 1********4321. \
                    Service charge of ETB 10.00 and VAT(15%) of ETB 1.50 and Disaster Recovery(5%) of 0.50 with total of ETB 512.00. \
                    Your current balance is ETB 984.42. Thanks for Banking with CBE. https://mbreciept.cbe.com.et/v2-aaaaaaaaaaaaaaaaaaaa";
        let sms = Sms { sender: "CBE", body, received_at: DateTime::parse_from_rfc3339("2026-11-07T19:54:00+03:00").unwrap() };
        match Parser::with_defaults().unwrap().parse(&sms) {
            Outcome::Parsed(p) => p,
            o => panic!("{o:?}"),
        }
    }

    // A real receipt's shape, names and numbers replaced.
    const TO_TELEBIRR: &str = r#"{"id":"FT26311AAAAA","transactionType":"ACNX","debitAccountNo":"1********4321",
        "debitAmount":"500.00","creditAccountNo":"E***********0162","paymentDetails":["TeleB0911223344"],
        "creditAccountHolder":"Tele Birr Settlement Account","debitAccountHolder":"Abebe Kebede Tesfaye",
        "isPlatformTransaction":true,"channel":"USSD","platformTransactionType":"WALLET_TRANSFER",
        "isOtherBank":false,"description":"TeleB0911223344","status":"COMPLETED"}"#;

    #[test]
    fn telebirr_top_up() {
        let mut p = cbe_debit();
        assert_eq!(wanted(&p), Some("v2-aaaaaaaaaaaaaaaaaaaa"));
        apply(&mut p, TO_TELEBIRR).unwrap();
        assert_eq!(p.kind, Kind::TransferOut);
        assert_eq!(p.counterparty.as_deref(), Some("telebirr"));
        assert_eq!(p.target_number.as_deref(), Some("0911223344"));
        assert_eq!(p.description.as_deref(), Some("wallet transfer"));
        assert!(p.receipt_checked);
        assert_eq!(wanted(&p), None, "looked up once");
    }

    #[test]
    fn transfer_to_a_person() {
        let mut p = cbe_debit();
        let json = r#"{"creditAccountHolder":"Hana Bekele Girma","creditAccountNo":"1********3414",
            "description":"for rent","platformTransactionType":"ACCOUNT_TRANSFER"}"#;
        apply(&mut p, json).unwrap();
        assert_eq!(p.kind, Kind::TransferOut);
        assert_eq!(p.counterparty.as_deref(), Some("Hana Bekele Girma"));
        assert_eq!(p.counterparty_account.as_deref(), Some("****3414"));
        assert_eq!(p.target_number, None);
    }

    #[test]
    fn never_overwrites_the_sms() {
        let mut p = cbe_debit();
        p.counterparty = Some("From the SMS".into());
        apply(&mut p, TO_TELEBIRR).unwrap();
        assert_eq!(p.counterparty.as_deref(), Some("From the SMS"));
    }

    #[test]
    fn branch_receipts_are_skipped() {
        let mut p = cbe_debit();
        p.reference = Some("FT26272AAAAA".into());
        assert_eq!(wanted(&p), None);
    }
}
