package dev.gravitaltalk.gravital_talk_app

import android.annotation.SuppressLint
import android.app.PendingIntent
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService

/**
 * Botón de Ajustes rápidos: hablar o soltar sin abrir la app, y volver a ella
 * cuando no hay sala.
 *
 * Desde la cortina no se puede mantener pulsado, así que, como la acción de
 * la notificación, un toque abre el micrófono y otro lo cierra. El estado lo
 * lee de [PttBubble], que guarda lo último que mandó Dart.
 */
class PttTileService : TileService() {
    override fun onStartListening() {
        super.onStartListening()
        render()
    }

    override fun onClick() {
        super.onClick()
        val toggle = SessionService.onTogglePtt
        if (PttBubble.mode == PttBubble.Mode.SESSION && !PttBubble.paused && toggle != null) {
            toggle()
            return
        }
        // Sin sala (o en pausa), el botón lleva a la app.
        open()
    }

    private fun render() {
        val tile = qsTile ?: return
        val live = PttBubble.mode == PttBubble.Mode.SESSION && SessionService.onTogglePtt != null
        val talking = live && PttBubble.talking
        tile.state = if (talking) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
        tile.label = when {
            !live -> "Gravital Talk"
            PttBubble.paused -> "Sala en pausa"
            talking -> "Soltar"
            else -> "Hablar"
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            tile.subtitle = when {
                !live -> "Sin sala"
                PttBubble.room.isNotEmpty() -> "Sala ${PttBubble.room}"
                else -> "En sala"
            }
        }
        tile.updateTile()
    }

    @SuppressLint("StartActivityAndCollapseDeprecated")
    private fun open() {
        val intent = Intent(this, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
        runCatching {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                startActivityAndCollapse(
                    PendingIntent.getActivity(this, 7, intent, PendingIntent.FLAG_IMMUTABLE),
                )
            } else {
                @Suppress("DEPRECATION")
                startActivityAndCollapse(intent)
            }
        }
    }

    companion object {
        /** Pide al sistema que repinte el botón con el estado nuevo. */
        fun refresh(context: Context) {
            runCatching {
                requestListeningState(context, ComponentName(context, PttTileService::class.java))
            }
        }
    }
}
