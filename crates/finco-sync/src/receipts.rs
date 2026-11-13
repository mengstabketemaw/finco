//! Fetching CBE online receipts (what they mean is `finco_core::receipt`).

use std::time::Duration;

use anyhow::{anyhow, Result};
use finco_core::receipt::{API, HEADERS};

/// The receipt's JSON, `None` if CBE has no such receipt, or an error to
/// retry later (offline, server trouble).
pub fn fetch(receipt_id: &str) -> Result<Option<String>> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .build();
    let mut req = agent.get(&format!("{API}{receipt_id}"));
    for (k, v) in HEADERS {
        req = req.set(k, v);
    }
    match req.call() {
        Ok(resp) => Ok(Some(resp.into_string()?)),
        Err(ureq::Error::Status(404 | 400 | 422, _)) => Ok(None),
        Err(ureq::Error::Status(code, _)) => Err(anyhow!("CBE receipt: HTTP {code}")),
        Err(e) => Err(anyhow!(e).context("can't reach CBE")),
    }
}
