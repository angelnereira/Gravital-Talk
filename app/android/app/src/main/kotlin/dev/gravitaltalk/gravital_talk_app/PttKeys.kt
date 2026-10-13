package dev.gravitaltalk.gravital_talk_app

import android.content.Context
import android.content.Intent
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.os.Build
import android.os.SystemClock
import android.view.KeyEvent

/**
 * Botones físicos como botón de hablar: el del auricular (con cable o
 * Bluetooth) y, si se activa, los de volumen.
 *
 * Mantener pulsado habla mientras dure; una pulsación corta abre el
 * micrófono y la siguiente lo cierra. Lo segundo no es un extra: muchos
 * auriculares sólo mandan el clic al soltar, y Android se reserva la
 * pulsación larga del botón de cable para el asistente de voz.
 *
 * El del auricular llega por una sesión de medios que existe sólo mientras
 * hay sala, y funciona también con la app oculta o la pantalla apagada. Los
 * de volumen sólo se oyen con la app a la vista ([MainActivity] se los pasa):
 * en segundo plano no hay forma de leerlos sin quitarle al teléfono el
 * control del volumen.
 */
object PttKeys {
    /** Abrir (`true`) o cerrar el micrófono. */
    var onPtt: ((Boolean) -> Unit)? = null

    private var session: MediaSession? = null
    private var inSession = false
    private var paused = false
    private var headset = true
    private var volume = false

    /** El micrófono está abierto, según lo último de Dart o lo pedido aquí. */
    private var open = false
    private var downAt = 0L
    private var wasOpen = false
    private var pressing = false

    fun update(context: Context, inSession: Boolean, talking: Boolean, paused: Boolean, headset: Boolean, volume: Boolean) {
        this.inSession = inSession
        this.paused = paused
        this.headset = headset
        this.volume = volume
        // Con un botón pulsado manda lo pedido aquí: lo de Dart puede llegar
        // un instante tarde.
        if (!pressing) open = talking
        if (inSession && headset) start(context) else stop()
    }

    fun release() {
        stop()
        onPtt = null
    }

    /** Botón de volumen con la app a la vista; `true` si se lo queda. */
    fun onVolumeKey(event: KeyEvent): Boolean {
        if (event.keyCode != KeyEvent.KEYCODE_VOLUME_DOWN && event.keyCode != KeyEvent.KEYCODE_VOLUME_UP) return false
        // El soltar de una pulsación ya empezada se atiende aunque la sala
        // acabe de pausarse: si no, el siguiente botón se quedaría colgado.
        if (!pressing && (!volume || !inSession || paused)) return false
        return handle(event)
    }

    private fun handle(event: KeyEvent): Boolean {
        when (event.action) {
            KeyEvent.ACTION_DOWN -> {
                if (!inSession || paused) return false
                // La tecla mantenida repite el evento: sólo cuenta el primero.
                if (event.repeatCount > 0 || pressing) return true
                pressing = true
                downAt = SystemClock.uptimeMillis()
                wasOpen = open
                if (!open) {
                    open = true
                    onPtt?.invoke(true)
                }
            }
            KeyEvent.ACTION_UP -> {
                if (!pressing) return true
                pressing = false
                val held = SystemClock.uptimeMillis() - downAt
                // Con el micrófono ya abierto, cualquier pulsación lo cierra;
                // si se abrió con ésta, sólo al soltar tras mantenerla.
                if (wasOpen || held >= HOLD_MS) {
                    open = false
                    onPtt?.invoke(false)
                }
            }
        }
        return true
    }

    private fun start(context: Context) {
        if (session != null) return
        session = runCatching {
            MediaSession(context.applicationContext, "gravital-talk-ptt").apply {
                if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
                    @Suppress("DEPRECATION")
                    setFlags(MediaSession.FLAG_HANDLES_MEDIA_BUTTONS)
                }
                setCallback(object : MediaSession.Callback() {
                    override fun onMediaButtonEvent(mediaButtonIntent: Intent): Boolean {
                        val event = keyEvent(mediaButtonIntent) ?: return false
                        if (event.keyCode !in HEADSET_KEYS) return false
                        return handle(event)
                    }
                })
                // Los botones van a la última sesión que "suena": sin estado
                // de reproducción se los llevaría el reproductor de música.
                setPlaybackState(
                    PlaybackState.Builder()
                        .setActions(
                            PlaybackState.ACTION_PLAY or PlaybackState.ACTION_PAUSE or
                                PlaybackState.ACTION_PLAY_PAUSE,
                        )
                        .setState(PlaybackState.STATE_PLAYING, PlaybackState.PLAYBACK_POSITION_UNKNOWN, 1f)
                        .build(),
                )
                isActive = true
            }
        }.getOrNull()
    }

    private fun stop() {
        session?.let { s ->
            runCatching {
                s.isActive = false
                s.release()
            }
        }
        session = null
        // Si la sala acaba con un botón pulsado, que no quede a medias.
        pressing = false
    }

    private fun keyEvent(intent: Intent): KeyEvent? = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        intent.getParcelableExtra(Intent.EXTRA_KEY_EVENT, KeyEvent::class.java)
    } else {
        @Suppress("DEPRECATION")
        intent.getParcelableExtra(Intent.EXTRA_KEY_EVENT)
    }

    /** Lo que hay que mantener para que soltar cierre el micrófono. */
    private const val HOLD_MS = 450L

    private val HEADSET_KEYS = setOf(
        KeyEvent.KEYCODE_HEADSETHOOK,
        KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE,
        KeyEvent.KEYCODE_MEDIA_PLAY,
        KeyEvent.KEYCODE_MEDIA_PAUSE,
    )
}
