package dev.finco.app

import android.content.Context

/**
 * The shared Rust core (`app/src-tauri/src/android.rs`): same library and
 * database as the Tauri UI, so the SMS receiver works with the app closed.
 */
object Core {
  init {
    System.loadLibrary("finco_app_lib")
  }

  @Volatile private var ready = false

  /** Call before anything else, from every entry point (activity, receivers). */
  fun ensureInit(ctx: Context) {
    if (ready) return
    synchronized(this) {
      if (!ready) {
        init(ctx.filesDir.absolutePath)
        ready = true
      }
    }
  }

  @JvmStatic external fun init(dir: String)

  /**
   * Store one SMS. `sentMs` is the time the SMS centre stamped it, which the
   * live receiver and the inbox both see the same way, so a message read
   * twice is stored once. Returns JSON `{outcome, ask}` or `{error}`.
   */
  @JvmStatic external fun ingest(sender: String, body: String, sentMs: Long, push: Boolean): String

  /** After reading the inbox: push what was found and refresh the UI. */
  @JvmStatic external fun scanDone()

  /** From a notification button. Returns an error message, or "". */
  @JvmStatic external fun categorize(id: String, category: String, remember: Boolean): String

  /** "Yes, paying back": `repayId` settles `settlesId`. Returns an error message, or "". */
  @JvmStatic external fun link(repayId: String, settlesId: String): String

  @JvmStatic external fun startSync()

  /** Called by Rust (any thread) after a local change or a sync pull. */
  @JvmStatic
  fun onNativeChange() {}
}
