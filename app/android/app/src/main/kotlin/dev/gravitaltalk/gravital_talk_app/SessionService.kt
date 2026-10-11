package dev.gravitaltalk.gravital_talk_app

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.os.PowerManager

/**
 * Mantiene viva la sala con la app en segundo plano.
 *
 * Sin un servicio en primer plano Android corta el micrófono y congela el
 * proceso a los pocos segundos de salir de la app: la conversación se
 * quedaría muda. El servicio no mueve audio; sólo le dice al sistema que hay
 * una conversación en curso y enseña la notificación desde la que volver o
 * salir.
 */
class SessionService : Service() {
    private var wakeLock: PowerManager.WakeLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_LEAVE) {
            onLeave?.invoke()
            stopSelf()
            return START_NOT_STICKY
        }
        val room = intent?.getStringExtra(EXTRA_ROOM).orEmpty()
        if (!enterForeground(notification(room))) {
            stopSelf()
            return START_NOT_STICKY
        }
        if (wakeLock == null) {
            // Con la pantalla apagada la CPU se duerme y el audio se corta.
            wakeLock = (getSystemService(Context.POWER_SERVICE) as? PowerManager)
                ?.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "gravital-talk:session")
                ?.apply { acquire() }
        }
        // Si el sistema mata el proceso no hay sesión que retomar.
        return START_NOT_STICKY
    }

    /**
     * Pasa a primer plano con los tipos que el sistema permita. El de
     * micrófono exige el permiso concedido: pedirlo sin tenerlo lanza una
     * excepción que cerraría la app, así que se prueba y se rebaja.
     */
    private fun enterForeground(notification: Notification): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
            return runCatching { startForeground(NOTIFICATION_ID, notification) }.isSuccess
        }
        val playback = ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK
        val micGranted = checkSelfPermission(Manifest.permission.RECORD_AUDIO) ==
            PackageManager.PERMISSION_GRANTED
        val attempts = if (micGranted && Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            listOf(playback or ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE, playback)
        } else {
            listOf(playback)
        }
        return attempts.any { types ->
            runCatching { startForeground(NOTIFICATION_ID, notification, types) }.isSuccess
        }
    }

    private fun notification(room: String): Notification {
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            manager.createNotificationChannel(
                NotificationChannel(CHANNEL_ID, "Sala en curso", NotificationManager.IMPORTANCE_LOW),
            )
        }
        val immutable = PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            immutable,
        )
        val leave = PendingIntent.getService(
            this,
            1,
            Intent(this, SessionService::class.java).setAction(ACTION_LEAVE),
            immutable,
        )
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }
        return builder
            .setSmallIcon(android.R.drawable.stat_sys_phone_call)
            .setContentTitle(if (room.isEmpty()) "Gravital Talk" else "Sala $room")
            .setContentText("Sigues escuchando. Toca para volver a hablar.")
            .setContentIntent(open)
            .setOngoing(true)
            .addAction(Notification.Action.Builder(null, "Salir de la sala", leave).build())
            .build()
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        // Cerrar la app desde recientes es salir de la sala.
        stopSelf()
    }

    override fun onDestroy() {
        wakeLock?.takeIf { it.isHeld }?.release()
        wakeLock = null
        super.onDestroy()
    }

    companion object {
        const val EXTRA_ROOM = "room"
        const val ACTION_LEAVE = "dev.gravitaltalk.gravital_talk_app.LEAVE"
        private const val CHANNEL_ID = "session"
        private const val NOTIFICATION_ID = 1

        /** Lo fija la actividad: salir desde la notificación lo decide Dart. */
        var onLeave: (() -> Unit)? = null
    }
}
