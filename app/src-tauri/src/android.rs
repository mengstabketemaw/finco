//! JNI entry points for the Kotlin side (`dev.finco.app.Core`): the SMS
//! receiver, the inbox scan and the notification buttons call these. They run
//! in the app's process, on the same store as the UI, with or without the UI
//! open.

use std::path::PathBuf;
use std::sync::OnceLock;

use jni::objects::{GlobalRef, JClass, JString};
use jni::sys::{jboolean, jlong, jstring};
use jni::{JNIEnv, JavaVM};

use crate::{changed, ensure_sync, fetch_receipts, parser, with_store};

/// The JVM and the `Core` class, to call `Core.onNativeChange()` from any thread.
static CALLBACK: OnceLock<(JavaVM, GlobalRef)> = OnceLock::new();

/// Point every file at the app's private directory. Must run before the
/// store or sync is used; repeated calls are harmless.
fn init(dir: &str) {
    let dir = PathBuf::from(dir);
    for (var, file) in [("FINCO_DB", "finco.db"), ("FINCO_SYNC_CONFIG", "sync.toml"), ("FINCO_SESSION", "session.json")] {
        if std::env::var_os(var).is_none() {
            std::env::set_var(var, dir.join(file));
        }
    }
}

/// Tell Kotlin the data changed (it refreshes its notification state).
pub(crate) fn notify_kotlin() {
    let Some((vm, class)) = CALLBACK.get() else { return };
    let Ok(mut env) = vm.attach_current_thread() else { return };
    let class: &JClass = class.as_obj().into();
    if env.call_static_method(class, "onNativeChange", "()V", &[]).is_err() {
        let _ = env.exception_clear();
    }
}

fn jstr(env: &mut JNIEnv, s: &str) -> jstring {
    env.new_string(s).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

fn rstr(env: &mut JNIEnv, s: &JString) -> String {
    env.get_string(s).map(Into::into).unwrap_or_default()
}

#[no_mangle]
pub extern "system" fn Java_dev_finco_app_Core_init<'l>(mut env: JNIEnv<'l>, class: JClass<'l>, dir: JString<'l>) {
    let dir = rstr(&mut env, &dir);
    init(&dir);
    if CALLBACK.get().is_none() {
        if let (Ok(vm), Ok(global)) = (env.get_java_vm(), env.new_global_ref(&class)) {
            let _ = CALLBACK.set((vm, global));
        }
    }
}

/// Store one SMS. Returns JSON [`finco_core::Ingested`] or `{"error": …}`.
/// `push` is false during the inbox scan, which pushes once at the end.
#[no_mangle]
pub extern "system" fn Java_dev_finco_app_Core_ingest<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    sender: JString<'l>,
    body: JString<'l>,
    received_ms: jlong,
    push: jboolean,
) -> jstring {
    let sender = rstr(&mut env, &sender);
    let body = rstr(&mut env, &body);
    let out = match with_store(|s| s.ingest(parser(), &sender, &body, received_ms)) {
        Ok(r) => {
            if push != 0 && r.outcome == "parsed" {
                changed();
                // A new CBE debit: find out where it went.
                if sender.trim().eq_ignore_ascii_case("CBE") {
                    fetch_receipts();
                }
            }
            serde_json::to_string(&r).unwrap_or_default()
        }
        Err(e) => serde_json::json!({ "error": format!("{e:#}") }).to_string(),
    };
    jstr(&mut env, &out)
}

/// After the inbox scan: push what it found and refresh the UI.
#[no_mangle]
pub extern "system" fn Java_dev_finco_app_Core_scanDone<'l>(_env: JNIEnv<'l>, _class: JClass<'l>) {
    changed();
    fetch_receipts();
}

/// From a notification button. Returns an error message, or "".
#[no_mangle]
pub extern "system" fn Java_dev_finco_app_Core_categorize<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    id: JString<'l>,
    category: JString<'l>,
    remember: jboolean,
) -> jstring {
    let id = rstr(&mut env, &id);
    let category = rstr(&mut env, &category);
    let msg = match with_store(|s| s.categorize(&id, Some(&category), remember != 0)) {
        Ok(_) => {
            changed();
            String::new()
        }
        Err(e) => format!("{e:#}"),
    };
    jstr(&mut env, &msg)
}

/// "Yes, paying back" on a notification. Returns an error message, or "".
#[no_mangle]
pub extern "system" fn Java_dev_finco_app_Core_link<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    repay_id: JString<'l>,
    settles_id: JString<'l>,
) -> jstring {
    let repay = rstr(&mut env, &repay_id);
    let settles = rstr(&mut env, &settles_id);
    let msg = match with_store(|s| s.link(&repay, &[settles])) {
        Ok(()) => {
            changed();
            String::new()
        }
        Err(e) => format!("{e:#}"),
    };
    jstr(&mut env, &msg)
}

#[no_mangle]
pub extern "system" fn Java_dev_finco_app_Core_startSync<'l>(_env: JNIEnv<'l>, _class: JClass<'l>) {
    ensure_sync();
}
