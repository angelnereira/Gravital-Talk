package dev.gravitaltalk.gravital_talk_app

import android.app.ActivityManager
import android.app.ApplicationExitInfo
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.view.KeyEvent
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
    private var background: MethodChannel? = null

    /** Enlace con el que se lanzó la app; se entrega una sola vez. */
    private var initialLink: String? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        recordUncaughtExceptions()
        // El audio nativo necesita el contexto de Android para preguntar por
        // los dispositivos. Si la librería no lo trae, la app usa sus plugins.
        runCatching {
            System.loadLibrary("gravital_talk_ffi")
            nativeInitAudio(applicationContext)
        }
    }

    private external fun nativeInitAudio(context: Context): Boolean

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
        this.background = background
        SessionService.onLeave = { runOnUiThread { background.invokeMethod("leave", null) } }
        SessionService.onTogglePtt = { runOnUiThread { background.invokeMethod("togglePtt", null) } }
        SessionService.onCallAction = { accept ->
            runOnUiThread { background.invokeMethod(if (accept) "answerCall" else "declineCall", null) }
        }
        val ptt: (Boolean) -> Unit = { down ->
            runOnUiThread { background.invokeMethod(if (down) "pttDown" else "pttUp", null) }
        }
        PttBubble.onPtt = ptt
        PttKeys.onPtt = ptt
        PttBubble.onResume = { runOnUiThread { background.invokeMethod("resume", null) } }
        background.setMethodCallHandler { call, result ->
            val service = Intent(this, SessionService::class.java)
            when (call.method) {
                "start" -> {
                    // Con la sala ya en marcha, sólo cambia el estado: se
                    // repinta la notificación sin volver a arrancar el servicio.
                    val status = call.argument<String>("status")
                    val running = SessionService.current
                    if (status != null && call.argument<String>("mode") != "standby" &&
                        running != null && running.refresh(call.argument<String>("room").orEmpty(), status)
                    ) {
                        result.success(true)
                        return@setMethodCallHandler
                    }
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
                "ring" -> {
                    if (call.argument<Boolean>("on") == true) Ringer.start(this) else Ringer.stop()
                    result.success(true)
                }
                "bubble" -> {
                    val mode = when (call.argument<String>("mode")) {
                        "session" -> PttBubble.Mode.SESSION
                        "ringing" -> PttBubble.Mode.RINGING
                        else -> PttBubble.Mode.OFF
                    }
                    val talking = call.argument<Boolean>("talking") == true
                    val paused = call.argument<Boolean>("paused") == true
                    PttBubble.update(
                        this,
                        mode,
                        talking = talking,
                        paused = paused,
                        peerTalking = call.argument<Boolean>("peerTalking") == true,
                        peerName = call.argument<String>("peerName").orEmpty(),
                        caller = call.argument<String>("caller").orEmpty(),
                        room = call.argument<String>("room").orEmpty(),
                        enabled = call.argument<Boolean>("enabled") != false,
                    )
                    PttKeys.update(
                        this,
                        inSession = mode == PttBubble.Mode.SESSION,
                        talking = talking,
                        paused = paused,
                        headset = call.argument<Boolean>("headset") != false,
                        volume = call.argument<Boolean>("volume") == true,
                    )
                    PttTileService.refresh(this)
                    result.success(true)
                }
                "moveToBack" -> result.success(moveTaskToBack(true))
                "notifyInfo" -> {
                    SessionService.notifyInfo(
                        this,
                        call.argument<String>("title").orEmpty(),
                        call.argument<String>("text").orEmpty(),
                    )
                    result.success(true)
                }
                else -> result.notImplemented()
            }
        }
    }

    /** Con la app a la vista, los botones de volumen pueden ser el de hablar. */
    override fun dispatchKeyEvent(event: KeyEvent): Boolean =
        PttKeys.onVolumeKey(event) || super.dispatchKeyEvent(event)

    override fun onStart() {
        super.onStart()
        PttBubble.setAppVisible(this, true)
    }

    override fun onStop() {
        // Fuera de la app, la burbuja toma el relevo del botón de hablar.
        PttBubble.setAppVisible(this, false)
        super.onStop()
    }

    override fun onDestroy() {
        // Sin actividad no hay Dart que mantenga la sesión.
        PttBubble.release()
        PttKeys.release()
        PttTileService.refresh(this)
        SessionService.onLeave = null
        SessionService.onTogglePtt = null
        SessionService.onCallAction = null
        Ringer.stop()
        stopService(Intent(this, SessionService::class.java))
        super.onDestroy()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        // La app ya estaba abierta: el enlace llega aquí, no al arranque.
        intent.pairingLink()?.let { channel?.invokeMethod("onLink", it) }
        // "Contestar" en la notificación de una llamada.
        // Desde la burbuja, además, se vuelve a lo que se estaba haciendo en
        // cuanto la sala conecta.
        if (intent.getBooleanExtra(SessionService.EXTRA_ANSWER, false)) {
            if (intent.getBooleanExtra(PttBubble.EXTRA_FROM_BUBBLE, false)) {
                background?.invokeMethod("answerFromBubble", null)
            } else {
                SessionService.onCallAction?.invoke(true)
            }
        }
    }

    /** Invitación que trae el intent: un enlace abierto o un texto compartido. */
    private fun Intent.pairingLink(): String? = when (action) {
        Intent.ACTION_VIEW -> dataString?.let { LINK.find(it)?.value }
        // Del mensaje compartido sólo interesa el enlace, no el texto que lo rodea.
        Intent.ACTION_SEND -> getStringExtra(Intent.EXTRA_TEXT)?.let { LINK.find(it)?.value }
        else -> null
    }

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

    private companion object {
        /**
         * Un enlace de la app, en su forma propia o en la web
         * (`http://localhost:47801/gravital-talk/…`). Dart lo pasa a la propia.
         */
        val LINK = Regex(
            "(?:gravital-talk://|https?://(?:localhost|127\\.0\\.0\\.1)(?::\\d+)?/gravital-talk/)\\S+",
        )
    }
}
