package dev.gravitaltalk.gravital_talk_app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/** "Ahora no" pulsado en la notificación de una llamada entrante. */
class CallActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        Ringer.stop()
        SessionService.cancelCall(context)
        SessionService.onCallAction?.invoke(false)
    }
}
