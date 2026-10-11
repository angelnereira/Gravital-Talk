package dev.gravitaltalk.gravital_talk_app

import android.app.ActivityManager
import android.app.ApplicationExitInfo
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Bundle
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import java.io.File

/**
 * Además de alojar Flutter, entrega a Dart las invitaciones
 * `gravital-talk://pair?…` con las que se abre la app y el motivo del último
 * cierre inesperado.
 */
class MainActivity : FlutterActivity() {
    private var channel: MethodChannel? = null

    /** Enlace con el que se lanzó la app; se entrega una sola vez. */
    private var initialLink: String? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        recordUncaughtExceptions()
    }

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        initialLink = intent?.pairingLink()
        channel = MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "gravital_talk/links").also {
            it.setMethodCallHandler { call, result ->
                when (call.method) {
                    "getInitialLink" -> {
                        result.success(initialLink)
                        initialLink = null
                    }
                    "share" -> {
                        val send = Intent(Intent.ACTION_SEND)
                            .setType("text/plain")
                            .putExtra(Intent.EXTRA_TEXT, call.argument<String>("text").orEmpty())
                        val shown = runCatching { startActivity(Intent.createChooser(send, null)) }.isSuccess
                        if (shown) result.success(true) else result.error("share", "sin panel de compartir", null)
                    }
                    else -> result.notImplemented()
                }
            }
        }
        bindBackground(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "gravital_talk/diagnostics")
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "filesDir" -> result.success(filesDir.absolutePath)
                    "lastExit" -> result.success(lastExit())
                    else -> result.notImplemented()
                }
            }
    }

    /** La sala sigue viva en segundo plano mientras dure: ver [SessionService]. */
    private fun bindBackground(flutterEngine: FlutterEngine) {
        val background = MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "gravital_talk/background")
        SessionService.onLeave = { runOnUiThread { background.invokeMethod("leave", null) } }
        SessionService.onTogglePtt = { runOnUiThread { background.invokeMethod("togglePtt", null) } }
        background.setMethodCallHandler { call, result ->
            val service = Intent(this, SessionService::class.java)
            when (call.method) {
                "start" -> {
                    service.putExtra(SessionService.EXTRA_ROOM, call.argument<String>("room").orEmpty())
                    service.putExtra(SessionService.EXTRA_STANDBY, call.argument<String>("mode") == "standby")
                    call.argument<String>("status")?.let { service.putExtra(SessionService.EXTRA_STATUS, it) }
                    // Arrancar el servicio puede fallar (app ya en segundo
                    // plano, restricciones del fabricante): la sala sigue,
                    // sólo que sin esa protección.
                    val started = runCatching {
                        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                            startForegroundService(service)
                        } else {
                            startService(service)
                        }
                    }.isSuccess
                    result.success(started)
                }
                "stop" -> {
                    stopService(service)
                    result.success(true)
                }
                "notifyCall" -> {
                    SessionService.notifyCall(this, call.argument<String>("name").orEmpty())
                    result.success(true)
                }
                "cancelCall" -> {
                    SessionService.cancelCall(this)
                    result.success(true)
                }
                else -> result.notImplemented()
            }
        }
    }

    override fun onDestroy() {
        // Sin actividad no hay Dart que mantenga la sesión.
        SessionService.onLeave = null
        SessionService.onTogglePtt = null
        stopService(Intent(this, SessionService::class.java))
        super.onDestroy()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        // La app ya estaba abierta: el enlace llega aquí, no al arranque.
        intent.pairingLink()?.let { channel?.invokeMethod("onLink", it) }
    }

    private fun Intent.pairingLink(): String? =
        dataString?.takeIf { action == Intent.ACTION_VIEW && it.startsWith("gravital-talk://") }

    /**
     * Anota la excepción que tumba el proceso antes de dejarla seguir: sin
     * esto, un fallo en la parte Android de un plugin no deja nada que leer.
     */
    private fun recordUncaughtExceptions() {
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        if (previous is Recorder) return
        Thread.setDefaultUncaughtExceptionHandler(Recorder(File(filesDir, "crash-java.txt"), previous))
    }

    private class Recorder(
        private val file: File,
        private val next: Thread.UncaughtExceptionHandler?,
    ) : Thread.UncaughtExceptionHandler {
        override fun uncaughtException(thread: Thread, error: Throwable) {
            try {
                file.writeText("hilo ${thread.name} · ${error.stackTraceToString().take(3000)}\n")
            } catch (_: Throwable) {
                // Sin sitio donde escribir: que al menos el proceso muera como siempre.
            }
            next?.uncaughtException(thread, error)
        }
    }

    /**
     * Motivo por el que el sistema dio por terminado el proceso anterior, si
     * fue un cierre que el usuario no pidió. Android lo guarda desde la 11.
     */
    private fun lastExit(): Map<String, Any>? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return null
        val manager = getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager ?: return null
        val info = manager.getHistoricalProcessExitReasons(packageName, 0, 1).firstOrNull() ?: return null
        val reason = when (info.reason) {
            ApplicationExitInfo.REASON_CRASH -> "excepción"
            ApplicationExitInfo.REASON_CRASH_NATIVE -> "fallo nativo"
            ApplicationExitInfo.REASON_ANR -> "la app dejó de responder"
            ApplicationExitInfo.REASON_LOW_MEMORY -> "el sistema se quedó sin memoria"
            ApplicationExitInfo.REASON_SIGNALED -> "señal del sistema"
            ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE -> "consumo excesivo de recursos"
            ApplicationExitInfo.REASON_INITIALIZATION_FAILURE -> "fallo al arrancar"
            else -> return null
        }
        val device = "${Build.MANUFACTURER} ${Build.MODEL} · Android ${Build.VERSION.RELEASE} · " +
            Build.SUPPORTED_ABIS.joinToString("/")
        val text = "sistema: $reason (estado ${info.status})" +
            (info.description?.let { " · $it" } ?: "") + " · $device"
        return mapOf("timestamp" to info.timestamp, "text" to text)
    }
}
