//! Supabase sync, copied from timlo's planner-sync and adapted.
//!
//! - [`config`]: project URL/key (`~/.config/finco/sync.toml`) and the login
//!   session (`~/.local/share/finco/session.json`, mode 0600).
//! - [`auth`]: email + password sign-in, sign-up and token refresh.
//! - [`client`]: push the local outbox and pull rows newer than the last pull.
//! - [`realtime`]: Supabase Realtime listener; any change triggers a pull.
//! - [`login`]: sign-in, sign-out and status for the CLI and the app.
//! - [`receipts`]: fetch CBE online receipts.
//! - [`worker`]: the background loop the desktop app or the Android app runs.
//!
//! Each device keeps working from its local SQLite copy; sync only keeps the
//! copies matched, last write wins per row (see `finco_core::sync`).

pub mod auth;
pub mod client;
pub mod config;
pub mod login;
pub mod realtime;
pub mod receipts;
pub mod worker;

pub use client::Client;
pub use config::{Session, SyncConfig};
pub use login::{login, logout, LoginOutcome, SyncStatus};
pub use worker::{start, SyncEvent};
