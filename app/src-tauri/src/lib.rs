//! Tauri backend: thin commands over `finco-core`, plus Supabase sync.
//!
//! One store, shared by the UI commands and (on Android) the SMS receiver,
//! which calls in over JNI from `android.rs` while the app may be closed.

#[cfg(target_os = "android")]
mod android;

use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};

use finco_core::{paths, Parser, Store, TxnView};
use finco_sync::{LoginOutcome, SyncConfig, SyncEvent};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

type CmdResult<T> = Result<T, String>;

static STORE: Mutex<Option<Store>> = Mutex::new(None);
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
static PARSER: OnceLock<Parser> = OnceLock::new();
static SYNC: Mutex<Option<Sender<SyncEvent>>> = Mutex::new(None);
static APP: OnceLock<AppHandle> = OnceLock::new();

/// Run `f` against the shared store, opening it on first use.
pub(crate) fn with_store<T>(f: impl FnOnce(&Store) -> anyhow::Result<T>) -> anyhow::Result<T> {
    let mut guard = STORE.lock().map_err(|_| anyhow::anyhow!("database lock poisoned"))?;
    if guard.is_none() {
        *guard = Some(Store::open(&paths::db_path())?);
    }
    f(guard.as_ref().unwrap())
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn parser() -> &'static Parser {
    PARSER.get_or_init(|| Parser::with_defaults().expect("bundled SMS rules are valid"))
}

fn cmd<T>(f: impl FnOnce(&Store) -> anyhow::Result<T>) -> CmdResult<T> {
    with_store(f).map_err(|e| format!("{e:#}"))
}

/// Something changed locally: push it, and tell the UI (a notification
/// button or a new SMS can change data while the screen is open).
pub(crate) fn changed() {
    if let Ok(guard) = SYNC.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(SyncEvent::Local);
        }
    }
    refresh_ui();
}

fn refresh_ui() {
    if let Some(app) = APP.get() {
        let _ = app.emit("data-changed", ());
    }
    #[cfg(target_os = "android")]
    android::notify_kotlin();
}

/// Start the sync threads if logged in and not already running.
pub(crate) fn ensure_sync() {
    let Ok(mut guard) = SYNC.lock() else { return };
    if guard.as_ref().is_some_and(|tx| tx.send(SyncEvent::Local).is_ok()) {
        return;
    }
    *guard = None;
    match finco_sync::start(paths::db_path(), refresh_ui) {
        Ok(tx) => *guard = tx,
        Err(e) => eprintln!("finco: sync not started: {e:#}"),
    }
}

/// Look up CBE online receipts not checked yet, in the background, once at
/// a time. Only the phone does this; the results reach the PC by sync. A
/// receipt that can't be fetched now (offline) is tried again next time.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn fetch_receipts() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        let mut changed_any = false;
        let todo = with_store(|s| s.receipts_to_fetch(100)).unwrap_or_default();
        for (message_id, receipt_id) in todo {
            // The network call holds no lock on the store.
            match finco_sync::receipts::fetch(&receipt_id) {
                Ok(json) => match with_store(|s| s.apply_receipt(&message_id, json.as_deref())) {
                    Ok(()) => changed_any = true,
                    Err(e) => eprintln!("finco: receipt {receipt_id}: {e:#}"),
                },
                Err(e) => {
                    eprintln!("finco: receipts paused: {e:#}");
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
        RUNNING.store(false, Ordering::SeqCst);
        if changed_any {
            changed();
        }
    });
}

/// Like `cmd`, then push the change.
fn write<T>(f: impl FnOnce(&Store) -> anyhow::Result<T>) -> CmdResult<T> {
    let out = cmd(f)?;
    changed();
    Ok(out)
}

// ── Data ────────────────────────────────────────────────────────────────────

#[tauri::command]
fn transactions() -> CmdResult<Vec<TxnView>> {
    cmd(|s| s.transactions())
}

#[tauri::command]
fn categorize(id: String, category: Option<String>, remember: bool) -> CmdResult<usize> {
    write(|s| s.categorize(&id, category.as_deref(), remember))
}

#[tauri::command]
fn link(repay_id: String, settles: Vec<String>) -> CmdResult<()> {
    write(|s| s.link(&repay_id, &settles))
}

#[tauri::command]
fn suggest(repay_id: String) -> CmdResult<Vec<finco_core::settle::Suggestion>> {
    cmd(|s| s.suggest(&repay_id))
}

#[tauri::command]
fn set_note(id: String, note: String) -> CmdResult<()> {
    write(|s| s.set_note(&id, &note))
}

#[tauri::command]
fn get_plans() -> CmdResult<Value> {
    cmd(|s| s.plans())
}

#[tauri::command]
fn save_plans(plans: Value) -> CmdResult<()> {
    write(|s| s.save_plans(&plans))
}

#[tauri::command]
fn get_settings() -> CmdResult<Value> {
    cmd(|s| s.settings())
}

#[tauri::command]
fn save_settings(settings: Value) -> CmdResult<()> {
    write(|s| s.save_settings(&settings))
}

#[derive(Serialize)]
struct SmsInfo {
    messages: usize,
    unrecognized: Vec<Value>,
    /// Whether this device reads SMS (only the phone does).
    reads_sms: bool,
}

#[tauri::command]
fn sms_info() -> CmdResult<SmsInfo> {
    cmd(|s| Ok(SmsInfo { messages: s.message_count()?, unrecognized: s.unrecognized()?, reads_sms: cfg!(target_os = "android") }))
}

// ── Sync ────────────────────────────────────────────────────────────────────

/// Defaults for the login form, from the repo's `.env` at build time.
const DEFAULT_SUPABASE_URL: &str = env!("FINCO_DEFAULT_SUPABASE_URL");
const DEFAULT_SUPABASE_KEY: &str = env!("FINCO_DEFAULT_SUPABASE_KEY");

#[derive(Serialize)]
struct SyncInfo {
    status: finco_sync::SyncStatus,
    default_url: String,
    default_key: String,
}

#[tauri::command]
async fn sync_status() -> CmdResult<SyncInfo> {
    let status = cmd(finco_sync::login::status)?;
    let saved = SyncConfig::load().ok().flatten();
    Ok(SyncInfo {
        status,
        default_url: saved.as_ref().map_or(DEFAULT_SUPABASE_URL.into(), |c| c.url.clone()),
        default_key: saved.map_or(DEFAULT_SUPABASE_KEY.into(), |c| c.anon_key),
    })
}

/// Runs off the UI thread (network). Returns a message for the user.
#[tauri::command]
async fn sync_login(url: String, key: String, email: String, password: String, signup: bool) -> CmdResult<String> {
    // Its own connection: the first sync can take a while and shouldn't
    // hold the shared store.
    let store = Store::open(&paths::db_path()).map_err(|e| format!("{e:#}"))?;
    let out = finco_sync::login(&store, SyncConfig::new(&url, &key), &email, &password, signup)
        .map_err(|e| format!("{e:#}"))?;
    Ok(match out {
        LoginOutcome::ConfirmEmail => "Account created. Click the link in the confirmation email, then log in.".into(),
        LoginOutcome::LoggedIn { sent, received } => {
            ensure_sync();
            refresh_ui();
            format!("Logged in. Sent {sent} rows, received {received}.")
        }
    })
}

#[tauri::command]
async fn sync_logout() -> CmdResult<()> {
    finco_sync::logout().map_err(|e| format!("{e:#}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();

    // A second launch focuses the window that is already open.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            use tauri::Manager;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }));
    }

    builder
        .setup(|app| {
            let _ = APP.set(app.handle().clone());
            with_store(|_| Ok(()))?;
            ensure_sync();
            #[cfg(target_os = "android")]
            fetch_receipts();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            transactions,
            categorize,
            set_note,
            link,
            suggest,
            get_plans,
            save_plans,
            get_settings,
            save_settings,
            sms_info,
            sync_status,
            sync_login,
            sync_logout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running finco");
}
