package dev.finco.app

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.provider.Telephony
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import org.json.JSONObject
import java.text.NumberFormat
import java.util.Locale

private const val TAG = "finco"

/** A new SMS arrived (the app may be closed): store it and ask about new spending. */
class SmsReceiver : BroadcastReceiver() {
  override fun onReceive(ctx: Context, intent: Intent) {
    if (intent.action != Telephony.Sms.Intents.SMS_RECEIVED_ACTION) return
    val parts = Telephony.Sms.Intents.getMessagesFromIntent(intent) ?: return
    // A long SMS arrives in parts; join them per sender.
    val messages = parts.groupBy { it.originatingAddress ?: "" }.map { (sender, ps) ->
      Triple(sender, ps.joinToString("") { it.messageBody ?: "" }, ps.first().timestampMillis)
    }
    val pending = goAsync()
    Thread {
      try {
        Core.ensureInit(ctx)
        Core.startSync()
        for ((sender, body, sentMs) in messages) {
          val out = JSONObject(Core.ingest(sender, body, sentMs, true))
          if (out.has("error")) Log.w(TAG, "ingest: ${out.getString("error")}")
          out.optJSONObject("ask")?.let { Ask.post(ctx, it) }
          // [money in, the payment it probably pays back]
          out.optJSONArray("repay")?.let { Ask.postRepay(ctx, it.getJSONObject(0), it.getJSONObject(1)) }
        }
      } catch (e: Throwable) {
        Log.e(TAG, "SMS receiver", e)
      } finally {
        pending.finish()
      }
    }.start()
  }
}

/** Reads the whole inbox once per app start; already-stored messages are skipped. */
object Inbox {
  @Volatile private var running = false

  fun scan(ctx: Context) {
    if (running) return
    if (ContextCompat.checkSelfPermission(ctx, android.Manifest.permission.READ_SMS) != PackageManager.PERMISSION_GRANTED) return
    running = true
    Thread {
      try {
        Core.ensureInit(ctx)
        var parsed = 0
        ctx.contentResolver.query(
          Telephony.Sms.Inbox.CONTENT_URI,
          arrayOf(Telephony.Sms.ADDRESS, Telephony.Sms.BODY, Telephony.Sms.DATE_SENT, Telephony.Sms.DATE),
          null, null, "${Telephony.Sms.DATE} ASC",
        )?.use { c ->
          while (c.moveToNext()) {
            val sender = c.getString(0) ?: continue
            val body = c.getString(1) ?: continue
            val sent = c.getLong(2).takeIf { it > 0 } ?: c.getLong(3)
            if (JSONObject(Core.ingest(sender, body, sent, false)).optString("outcome") == "parsed") parsed++
          }
        }
        Log.i(TAG, "inbox scan: $parsed new")
        if (parsed > 0) Core.scanDone()
      } catch (e: Throwable) {
        Log.e(TAG, "inbox scan", e)
      } finally {
        running = false
      }
    }.start()
  }
}

/** The "What was this?" notification after a payment. */
object Ask {
  private const val CHANNEL = "ask"
  const val EXTRA_ID = "txn_id"
  const val EXTRA_CATEGORY = "category"
  const val EXTRA_NOTIFICATION = "notification"

  const val EXTRA_SETTLES = "settles"

  /** Three quick answers; anything else is a tap away in the app. */
  private val QUICK = listOf("food" to "🍲 Food", "snacks" to "☕ Snacks", "lent" to "🤲 Lent")

  private fun allowed(ctx: Context): Boolean {
    if (Build.VERSION.SDK_INT >= 33 &&
      ContextCompat.checkSelfPermission(ctx, android.Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
    ) return false
    val nm = ctx.getSystemService(NotificationManager::class.java)
    if (Build.VERSION.SDK_INT >= 26 && nm.getNotificationChannel(CHANNEL) == null) {
      nm.createNotificationChannel(
        NotificationChannel(CHANNEL, "What was this?", NotificationManager.IMPORTANCE_DEFAULT).apply {
          description = "Asks what each payment was for, and whether money in pays something back"
        }
      )
    }
    return true
  }

  private fun birr(santim: Long): String = NumberFormat.getNumberInstance(Locale.US).apply {
    minimumFractionDigits = 2; maximumFractionDigits = 2
  }.format(santim / 100.0)

  private fun who(txn: JSONObject, fallback: String): String =
    txn.optString("counterparty").takeIf { it.isNotEmpty() && it != "null" }
      ?: txn.optString("description").takeIf { it.isNotEmpty() && it != "null" }
      ?: fallback

  private fun openApp(ctx: Context): PendingIntent = PendingIntent.getActivity(
    ctx, 0,
    Intent(ctx, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP),
    PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
  )

  /** Money in that probably pays back `payment`: "Yes, paying back" links them. */
  fun postRepay(ctx: Context, repay: JSONObject, suggestion: JSONObject) {
    if (!allowed(ctx)) return
    val payment = suggestion.getJSONObject("txn")
    val id = repay.getString("id")
    val notificationId = id.hashCode()
    val day = payment.getString("occurred_at").substring(5, 10).split("-").let { (m, d) -> "$d/$m" }
    val yes = Intent(ctx, AnswerReceiver::class.java)
      .putExtra(EXTRA_ID, id)
      .putExtra(EXTRA_SETTLES, payment.getString("id"))
      .putExtra(EXTRA_NOTIFICATION, notificationId)
    val b = NotificationCompat.Builder(ctx, CHANNEL)
      .setSmallIcon(R.drawable.ic_notify)
      .setContentTitle("${who(repay, "Someone")} sent ETB ${birr(repay.getLong("amount"))}")
      .setContentText("Paying you back for ETB ${birr(payment.getLong("amount"))} on $day?")
      .setContentIntent(openApp(ctx))
      .setAutoCancel(true)
      .addAction(0, "Yes, paying back", PendingIntent.getBroadcast(
        ctx, notificationId * 4, yes, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE))
      .addAction(0, "No, it's income", PendingIntent.getBroadcast(
        ctx, notificationId * 4 + 1,
        Intent(ctx, AnswerReceiver::class.java).putExtra(EXTRA_NOTIFICATION, notificationId),
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE))
    NotificationManagerCompat.from(ctx).notify(notificationId, b.build())
  }

  fun post(ctx: Context, txn: JSONObject) {
    if (!allowed(ctx)) return
    val id = txn.getString("id")
    val notificationId = id.hashCode()
    val amount = birr(txn.getLong("amount"))
    val who = who(txn, "a payment")
    val open = openApp(ctx)
    val b = NotificationCompat.Builder(ctx, CHANNEL)
      .setSmallIcon(R.drawable.ic_notify)
      .setContentTitle("ETB $amount to $who")
      .setContentText("What was this? Tap for more categories.")
      .setContentIntent(open)
      .setAutoCancel(true)
      .setCategory(NotificationCompat.CATEGORY_REMINDER)
    QUICK.forEachIndexed { i, (category, label) ->
      val answer = Intent(ctx, AnswerReceiver::class.java)
        .putExtra(EXTRA_ID, id)
        .putExtra(EXTRA_CATEGORY, category)
        .putExtra(EXTRA_NOTIFICATION, notificationId)
      val pi = PendingIntent.getBroadcast(
        ctx, notificationId * 4 + i, answer, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
      )
      b.addAction(0, label, pi)
    }
    NotificationManagerCompat.from(ctx).notify(notificationId, b.build())
  }
}

/**
 * A notification button: tag the payment (remembering the recipient, except
 * for "Lent", which is about this one payment), or link money in to the
 * payment it pays back, or just dismiss.
 */
class AnswerReceiver : BroadcastReceiver() {
  override fun onReceive(ctx: Context, intent: Intent) {
    val id = intent.getStringExtra(Ask.EXTRA_ID)
    val category = intent.getStringExtra(Ask.EXTRA_CATEGORY)
    val settles = intent.getStringExtra(Ask.EXTRA_SETTLES)
    val notificationId = intent.getIntExtra(Ask.EXTRA_NOTIFICATION, 0)
    val pending = goAsync()
    Thread {
      try {
        Core.ensureInit(ctx)
        Core.startSync()
        val err = when {
          id != null && category != null -> Core.categorize(id, category, category != "lent")
          id != null && settles != null -> Core.link(id, settles)
          else -> ""
        }
        if (err.isNotEmpty()) Log.w(TAG, "answer: $err")
        NotificationManagerCompat.from(ctx).cancel(notificationId)
      } finally {
        pending.finish()
      }
    }.start()
  }
}
