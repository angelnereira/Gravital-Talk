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
import android.net.wifi.WifiManager
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
    private var wifiLocks: List<WifiManager.WifiLock> = emptyList()

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_LEAVE) {
            onLeave?.invoke()
            stopSelf()
            return START_NOT_STICKY
        }
        if (intent?.action == ACTION_TOGGLE_PTT) {
            // Lo decide Dart, que es quien tiene la sesión; después vuelve
            // por aquí con el estado nuevo para repintar la notificación.
            onTogglePtt?.invoke()
            return START_NOT_STICKY
        }
        val room = intent?.getStringExtra(EXTRA_ROOM).orEmpty()
        val standby = intent?.getBooleanExtra(EXTRA_STANDBY, false) ?: false
        val status = intent?.getStringExtra(EXTRA_STATUS) ?: STATUS_LISTENING
        // Primero la de estilo llamada; si el sistema la rechaza, la sencilla.
        val candidates = listOfNotNull(
            if (standby) null else callNotification(room, status),
            notification(room, standby, status),
        )
        if (candidates.none { enterForeground(it) }) {
            stopSelf()
            return START_NOT_STICKY
        }
        inSession = !standby
        current = this
        if (standby) {
            // A la espera de llamadas no se retiene la CPU: sólo el proceso.
            wakeLock?.takeIf { it.isHeld }?.release()
            wakeLock = null
            releaseWifi()
        } else if (wakeLock == null) {
            holdWifi()
            // Con la pantalla apagada la CPU se duerme y el audio se corta.
            wakeLock = (getSystemService(Context.POWER_SERVICE) as? PowerManager)
                ?.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "gravital-talk:session")
                ?.apply { acquire() }
        }
        // Si el sistema mata el proceso no hay sesión que retomar.
        return START_NOT_STICKY
    }

    /** `true` con una sala en curso (no en la espera de llamadas). */
    var inSession = false
        private set

    /**
     * Cambia sólo el texto y las acciones de la notificación de la sala.
     *
     * No vuelve a pasar por `startForeground`: hacerlo con la app oculta (al
     * hablar desde la burbuja) obliga al sistema a revisar otra vez el tipo
     * micrófono, y fuera de las excepciones (tocar la notificación) lo rebaja
     * y el micrófono deja de captar. Devuelve `false` si no pudo.
     */
    fun refresh(room: String, status: String): Boolean {
        if (!inSession) return false
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val candidates = listOfNotNull(callNotification(room, status), notification(room, false, status))
        return candidates.any { runCatching { manager.notify(NOTIFICATION_ID, it) }.isSuccess }
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

    private fun statusText(status: String) = when (status) {
        STATUS_TALKING -> "Estás hablando."
        STATUS_PAUSED -> "Sala en pausa."
        else -> "Sigues escuchando. Toca para volver a la sala."
    }

    private fun serviceIntent(code: Int, action: String): PendingIntent = PendingIntent.getService(
        this,
        code,
        Intent(this, SessionService::class.java).setAction(action),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    /** Acción de hablar o soltar; `null` en pausa, donde no se puede hablar. */
    private fun pttAction(status: String): Notification.Action? {
        if (status == STATUS_PAUSED) return null
        val label = if (status == STATUS_TALKING) "Soltar" else "Hablar"
        return Notification.Action.Builder(null, label, serviceIntent(3, ACTION_TOGGLE_PTT)).build()
    }

    /**
     * Notificación con aspecto de llamada en curso (Android 12+): el sistema
     * la coloca arriba y le pone el botón de colgar. `null` en versiones
     * anteriores.
     */
    private fun callNotification(room: String, status: String): Notification? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) return null
        return runCatching {
            val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            manager.createNotificationChannel(
                NotificationChannel(CHANNEL_ID, "Sala en curso", NotificationManager.IMPORTANCE_LOW),
            )
            val open = PendingIntent.getActivity(
                this,
                0,
                Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
            val who = android.app.Person.Builder()
                .setName(if (room.isEmpty()) "Gravital Talk" else "Sala $room")
                .setImportant(true)
                .build()
            val builder = Notification.Builder(this, CHANNEL_ID)
                .setSmallIcon(android.R.drawable.stat_sys_phone_call)
                .setContentText(statusText(status))
                .setContentIntent(open)
                .setOngoing(true)
                .setCategory(Notification.CATEGORY_CALL)
                .setStyle(Notification.CallStyle.forOngoingCall(who, serviceIntent(1, ACTION_LEAVE)))
            pttAction(status)?.let { builder.addAction(it) }
            builder.build()
        }.getOrNull()
    }

    private fun notification(room: String, standby: Boolean, status: String): Notification {
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
        builder
            .setSmallIcon(android.R.drawable.stat_sys_phone_call)
            .setContentIntent(open)
            .setOngoing(true)
        if (standby) {
            return builder
                .setContentTitle("Gravital Talk")
                .setContentText("Disponible para tus contactos.")
                .build()
        }
        builder
            .setContentTitle(if (room.isEmpty()) "Gravital Talk" else "Sala $room")
            .setContentText(statusText(status))
        pttAction(status)?.let { builder.addAction(it) }
        return builder
            .addAction(Notification.Action.Builder(null, "Salir de la sala", leave).build())
            .build()
    }

    /**
     * Impide que el wifi ahorre energía mientras dura la sala. En reposo la
     * radio duerme entre balizas y entrega lo recibido a ráfagas: tras unos
     * segundos de silencio, el principio de cada turno llegaba a trompicones.
     */
    private fun holdWifi() {
        if (wifiLocks.isNotEmpty()) return
        val wifi = applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager ?: return
        val modes = buildList {
            // Baja latencia sólo actúa con la app a la vista; alto rendimiento
            // cubre el segundo plano.
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) add(WifiManager.WIFI_MODE_FULL_LOW_LATENCY)
            @Suppress("DEPRECATION")
            add(WifiManager.WIFI_MODE_FULL_HIGH_PERF)
        }
        wifiLocks = modes.mapNotNull { mode ->
            runCatching {
                wifi.createWifiLock(mode, "gravital-talk:session").apply {
                    setReferenceCounted(false)
                    acquire()
                }
            }.getOrNull()
        }
    }

    private fun releaseWifi() {
        wifiLocks.forEach { lock -> runCatching { if (lock.isHeld) lock.release() } }
        wifiLocks = emptyList()
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        // Cerrar la app desde recientes es salir de la sala.
        stopSelf()
    }

    override fun onDestroy() {
        if (current === this) current = null
        inSession = false
        wakeLock?.takeIf { it.isHeld }?.release()
        wakeLock = null
        releaseWifi()
        super.onDestroy()
    }

    companion object {
        const val EXTRA_ROOM = "room"
        const val EXTRA_STANDBY = "standby"
        const val EXTRA_STATUS = "status"
        const val STATUS_LISTENING = "listening"
        const val STATUS_TALKING = "talking"
        const val STATUS_PAUSED = "paused"
        const val ACTION_TOGGLE_PTT = "dev.gravitaltalk.gravital_talk_app.TOGGLE_PTT"
        const val ACTION_LEAVE = "dev.gravitaltalk.gravital_talk_app.LEAVE"
        private const val CHANNEL_ID = "session"
        private const val NOTIFICATION_ID = 1

        // Nuevo id: un canal no cambia de sonido una vez creado.
        private const val CALL_CHANNEL_ID = "calls.silent"
        private const val CALL_NOTIFICATION_ID = 2

        private const val INFO_CHANNEL_ID = "info"
        private const val INFO_NOTIFICATION_ID = 3

        const val EXTRA_ANSWER = "dev.gravitaltalk.gravital_talk_app.ANSWER"

        private fun builder(context: Context, channel: String, name: String, importance: Int): Notification.Builder {
            val manager = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
                @Suppress("DEPRECATION")
                return Notification.Builder(context)
            }
            manager.createNotificationChannel(
                NotificationChannel(channel, name, importance).apply {
                    if (channel == CALL_CHANNEL_ID) {
                        // El tono y la vibración los pone [Ringer]: así son
                        // los mismos con la app a la vista y se pueden parar.
                        setSound(null, null)
                        enableVibration(false)
                    } else {
                        enableVibration(true)
                        vibrationPattern = longArrayOf(0, 40, 80, 40)
                    }
                },
            )
            return Notification.Builder(context, channel)
        }

        /**
         * Avisa de que un contacto quiere hablar. Tocarla abre la app;
         * "Contestar" además entra en su sala y "Ahora no" la rechaza.
         */
        fun notifyCall(context: Context, name: String) {
            val manager = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            val flags = PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
            val activity = Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
            val open = PendingIntent.getActivity(context, 2, activity, flags)
            val answer = PendingIntent.getActivity(
                context,
                4,
                Intent(activity).putExtra(EXTRA_ANSWER, true),
                flags,
            )
            val decline = PendingIntent.getBroadcast(
                context,
                5,
                Intent(context, CallActionReceiver::class.java),
                flags,
            )
            // Sin permiso de notificaciones no se enseña: la solicitud espera
            // dentro de la app.
            runCatching {
                manager.notify(
                    CALL_NOTIFICATION_ID,
                    builder(context, CALL_CHANNEL_ID, "Llamadas de contactos", NotificationManager.IMPORTANCE_HIGH)
                        .setSmallIcon(android.R.drawable.stat_sys_phone_call)
                        .setContentTitle("$name quiere hablar")
                        .setContentText("Toca para contestar.")
                        .setCategory(Notification.CATEGORY_CALL)
                        .setContentIntent(open)
                        .setAutoCancel(true)
                        .addAction(Notification.Action.Builder(null, "Ahora no", decline).build())
                        .addAction(Notification.Action.Builder(null, "Contestar", answer).build())
                        .build(),
                )
            }
        }

        /** Aviso corriente (alguien aceptó tu invitación…); tocarlo abre la app. */
        fun notifyInfo(context: Context, title: String, text: String) {
            val manager = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            val open = PendingIntent.getActivity(
                context,
                6,
                Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
            runCatching {
                manager.notify(
                    INFO_NOTIFICATION_ID,
                    builder(context, INFO_CHANNEL_ID, "Avisos", NotificationManager.IMPORTANCE_DEFAULT)
                        .setSmallIcon(android.R.drawable.stat_notify_chat)
                        .setContentTitle(title)
                        .setContentText(text)
                        .setContentIntent(open)
                        .setAutoCancel(true)
                        .build(),
                )
            }
        }

        fun cancelCall(context: Context) {
            (context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager)
                .cancel(CALL_NOTIFICATION_ID)
        }

        /** El servicio en marcha, si lo hay: para repintar sin rearrancarlo. */
        @Volatile
        var current: SessionService? = null
            private set

        /** Lo fija la actividad: salir desde la notificación lo decide Dart. */
        var onLeave: (() -> Unit)? = null

        /** Hablar o soltar desde la notificación. */
        var onTogglePtt: (() -> Unit)? = null

        /** Contestar (`true`) o rechazar la llamada desde su notificación. */
        var onCallAction: ((Boolean) -> Unit)? = null
    }
}
