package dev.gravitaltalk.gravital_talk_app

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioManager
import android.media.MediaPlayer
import android.os.Build
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager

/**
 * Tono y vibración de una llamada entrante.
 *
 * Suena aquí y no como sonido de la notificación para que sea el mismo con
 * la app a la vista y oculta, y para poder pararlo en cuanto se contesta.
 * Respeta el modo del teléfono: en silencio no suena, en vibración sólo
 * vibra.
 */
object Ringer {
    private var player: MediaPlayer? = null
    private var vibrator: Vibrator? = null

    @Synchronized
    fun start(context: Context) {
        if (player != null || vibrator != null) return
        val app = context.applicationContext
        val mode = (app.getSystemService(Context.AUDIO_SERVICE) as? AudioManager)?.ringerMode
            ?: AudioManager.RINGER_MODE_NORMAL
        if (mode == AudioManager.RINGER_MODE_SILENT) return

        // Dos pulsos cortos y una pausa, al compás del tono.
        val pattern = longArrayOf(0, 70, 110, 70, 2350)
        // Marcado como timbre: sin esto hay teléfonos que no vibran con la
        // app en segundo plano.
        val ringtone = AudioAttributes.Builder()
            .setUsage(AudioAttributes.USAGE_NOTIFICATION_RINGTONE)
            .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
            .build()
        vibrator = vibratorOf(app)?.also { v ->
            runCatching {
                @Suppress("DEPRECATION")
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    v.vibrate(VibrationEffect.createWaveform(pattern, 0), ringtone)
                } else {
                    v.vibrate(pattern, 0, ringtone)
                }
            }
        }
        if (mode != AudioManager.RINGER_MODE_NORMAL) return
        player = runCatching {
            MediaPlayer().apply {
                setAudioAttributes(ringtone)
                app.resources.openRawResourceFd(R.raw.gt_ring).use {
                    setDataSource(it.fileDescriptor, it.startOffset, it.length)
                }
                isLooping = true
                prepare()
                start()
            }
        }.getOrNull()
    }

    @Synchronized
    fun stop() {
        player?.let { p -> runCatching { p.stop() }; runCatching { p.release() } }
        player = null
        vibrator?.let { v -> runCatching { v.cancel() } }
        vibrator = null
    }

    private fun vibratorOf(context: Context): Vibrator? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            (context.getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as? VibratorManager)
                ?.defaultVibrator
        } else {
            @Suppress("DEPRECATION")
            context.getSystemService(Context.VIBRATOR_SERVICE) as? Vibrator
        }
}
