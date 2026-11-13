//! File locations. Each can be overridden with an environment variable (the
//! Android app points them at its private directory).

use std::path::PathBuf;

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// `$FINCO_DB`, else `~/.local/share/finco/finco.db`.
pub fn db_path() -> PathBuf {
    std::env::var_os("FINCO_DB").map(PathBuf::from).unwrap_or_else(|| {
        dirs::data_dir().unwrap_or_else(|| home().join(".local/share")).join("finco/finco.db")
    })
}
