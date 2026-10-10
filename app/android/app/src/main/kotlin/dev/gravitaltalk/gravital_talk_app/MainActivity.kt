package dev.gravitaltalk.gravital_talk_app

import android.content.Intent
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

/**
 * Además de alojar Flutter, entrega a Dart las invitaciones
 * `gravital-talk://pair?…` con las que se abre la app.
 */
class MainActivity : FlutterActivity() {
    private var channel: MethodChannel? = null

    /** Enlace con el que se lanzó la app; se entrega una sola vez. */
    private var initialLink: String? = null

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        initialLink = intent?.pairingLink()
        channel = MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "gravital_talk/links").also {
            it.setMethodCallHandler { call, result ->
                if (call.method == "getInitialLink") {
                    result.success(initialLink)
                    initialLink = null
                } else {
                    result.notImplemented()
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        // La app ya estaba abierta: el enlace llega aquí, no al arranque.
        intent.pairingLink()?.let { channel?.invokeMethod("onLink", it) }
    }

    private fun Intent.pairingLink(): String? =
        dataString?.takeIf { action == Intent.ACTION_VIEW && it.startsWith("gravital-talk://") }
}
