package dev.finco.app

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    // Before the Tauri side opens the database.
    Core.ensureInit(this)
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    keepClearOfSystemBars()

    val wanted = mutableListOf(Manifest.permission.RECEIVE_SMS, Manifest.permission.READ_SMS)
    if (Build.VERSION.SDK_INT >= 33) wanted += Manifest.permission.POST_NOTIFICATIONS
    val missing = wanted.filter { ContextCompat.checkSelfPermission(this, it) != PackageManager.PERMISSION_GRANTED }
    if (missing.isNotEmpty()) ActivityCompat.requestPermissions(this, missing.toTypedArray(), 1)
    else Inbox.scan(this)
  }

  override fun onResume() {
    super.onResume()
    // Catch anything that arrived while the receiver couldn't run.
    Inbox.scan(this)
  }

  override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
    super.onRequestPermissionsResult(requestCode, permissions, grantResults)
    Inbox.scan(this)
  }

  /**
   * Edge-to-edge is forced on Android 15+, so the web view would sit under the
   * status bar, camera cutout and navigation bar. Pad it by their size (and
   * the keyboard's, so focused inputs stay visible).
   */
  private fun keepClearOfSystemBars() {
    val content = findViewById<android.view.View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { v, insets ->
      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout() or WindowInsetsCompat.Type.ime()
      )
      v.setPadding(bars.left, bars.top, bars.right, bars.bottom)
      WindowInsetsCompat.CONSUMED
    }
    ViewCompat.requestApplyInsets(content)
  }
}
