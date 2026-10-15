package invalid.placeholder.arya_vault

import android.app.Activity
import android.content.BroadcastReceiver
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Build
import android.os.PersistableBundle
import android.view.WindowManager
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.security.SecureRandom

/**
 * Android side of the `aryavault/platform` channel (app/lib/src/services/windows_platform.dart
 * holds the Dart side). Same method names and argument shapes as the Windows runner, so the
 * Dart services are shared. No logic lives here beyond what the OS forces: the clipboard timing
 * and the lock decisions are ClipboardManager and AutoLockController in Dart.
 *
 * Implemented: clipboard.write / fingerprint / clear (SEC-H01), capture.setBlocked (SEC-H02,
 * FLAG_SECURE) and the screenLocked / screenUnlocked / sleep / resume events.
 * Not yet: file.save / file.pick (storage access framework).
 */
class PlatformChannel(
    private val activity: Activity,
    messenger: BinaryMessenger,
) : MethodChannel.MethodCallHandler {
    private val channel = MethodChannel(messenger, CHANNEL_NAME)
    private val clipboard =
        activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

    // Random per process: the fingerprint is only ever compared with another one of the same
    // run, and a salted hash keeps the clip content out of the value Dart holds in memory.
    private val salt = ByteArray(16).also { SecureRandom().nextBytes(it) }

    private val screenReceiver =
        object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                when (intent.action) {
                    Intent.ACTION_SCREEN_OFF -> {
                        send("screenLocked")
                        send("sleep")
                    }
                    Intent.ACTION_USER_PRESENT -> {
                        send("screenUnlocked")
                        send("resume")
                    }
                }
            }
        }

    init {
        channel.setMethodCallHandler(this)
        val filter = IntentFilter().apply {
            addAction(Intent.ACTION_SCREEN_OFF)
            addAction(Intent.ACTION_USER_PRESENT)
        }
        // Protected system broadcasts, so the receiver need not be exported.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            activity.registerReceiver(screenReceiver, filter, Context.RECEIVER_NOT_EXPORTED)
        } else {
            @Suppress("UnspecifiedRegisterReceiverFlag")
            activity.registerReceiver(screenReceiver, filter)
        }
    }

    fun dispose() {
        channel.setMethodCallHandler(null)
        try {
            activity.unregisterReceiver(screenReceiver)
        } catch (_: IllegalArgumentException) {
            // Already unregistered.
        }
    }

    private fun send(event: String) = channel.invokeMethod("event", event)

    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "clipboard.write" -> writeClipboard(call, result)
            "clipboard.fingerprint" -> result.success(fingerprint())
            "clipboard.clear" -> clearClipboard(result)
            "capture.setBlocked" -> setCaptureBlocked(call, result)
            else -> result.notImplemented()
        }
    }

    private fun writeClipboard(call: MethodCall, result: MethodChannel.Result) {
        val bytes = call.argument<ByteArray>("bytes")
        val sensitive = call.argument<Boolean>("sensitive")
        if (bytes == null || sensitive == null) {
            result.error("bad_args", "bytes and sensitive are required", null)
            return
        }
        try {
            val clip = ClipData.newPlainText("", String(bytes, StandardCharsets.UTF_8))
            if (sensitive) {
                // Keeps the value out of the clipboard preview and, where the OS honours it,
                // out of clipboard history and sync (SEC-H01). The key is the same string on
                // Android 13+ (ClipDescription.EXTRA_IS_SENSITIVE) and in the earlier
                // vendor-recognised form.
                clip.description.extras =
                    PersistableBundle().apply { putBoolean(EXTRA_IS_SENSITIVE, true) }
            }
            clipboard.setPrimaryClip(clip)
            result.success(null)
        } catch (e: RuntimeException) {
            result.error("clipboard_failed", "could not write the clipboard", null)
        } finally {
            bytes.fill(0)
        }
    }

    // Android has no clipboard sequence number. The salted hash of the text identifies "the
    // thing we put there" and nothing else. Null when the clipboard is empty, not text, or
    // not readable (Android 10+ only lets the focused app read it).
    private fun fingerprint(): String? {
        return try {
            val clip = clipboard.primaryClip ?: return null
            if (clip.itemCount == 0) return null
            val text = clip.getItemAt(0).text ?: return null
            val digest = MessageDigest.getInstance("SHA-256")
            digest.update(salt)
            digest.update(text.toString().toByteArray(StandardCharsets.UTF_8))
            "sha256:" + digest.digest().joinToString("") { "%02x".format(it) }
        } catch (e: RuntimeException) {
            null
        }
    }

    private fun clearClipboard(result: MethodChannel.Result) {
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                clipboard.clearPrimaryClip()
            } else {
                clipboard.setPrimaryClip(ClipData.newPlainText("", ""))
            }
            result.success(null)
        } catch (e: RuntimeException) {
            result.error("clipboard_failed", "could not clear the clipboard", null)
        }
    }

    private fun setCaptureBlocked(call: MethodCall, result: MethodChannel.Result) {
        val blocked = call.arguments as? Boolean
        if (blocked == null) {
            result.error("bad_args", "expected a bool", null)
            return
        }
        activity.runOnUiThread {
            if (blocked) {
                activity.window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
            } else {
                activity.window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
            }
            result.success(null)
        }
    }

    private companion object {
        const val CHANNEL_NAME = "aryavault/platform"
        const val EXTRA_IS_SENSITIVE = "android.content.extra.IS_SENSITIVE"
    }
}
