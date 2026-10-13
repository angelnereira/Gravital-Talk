package dev.gravitaltalk.gravital_talk_app

import android.animation.ValueAnimator
import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.RectF
import android.os.Build
import android.provider.Settings
import android.view.Gravity
import android.view.HapticFeedbackConstants
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.view.WindowManager
import android.view.animation.DecelerateInterpolator
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min

/**
 * Burbuja flotante para hablar sin volver a la app.
 *
 * Aparece encima de las demás apps cuando la app no está a la vista y hay una
 * sala en curso o una llamada entrante. Mantenerla pulsada es el botón de
 * hablar; tocarla abre la app (o contesta la llamada); arrastrarla la mueve, y
 * soltarla sobre la equis de abajo la quita hasta la próxima vez que se salga
 * de la app.
 *
 * Quien decide hablar sigue siendo Dart: la burbuja sólo avisa de que se
 * pulsó o se soltó ([onPtt]) y pinta el estado que Dart le devuelve. Necesita
 * el permiso de mostrar encima de otras apps; sin él no aparece y la
 * notificación sigue siendo el único control.
 */
object PttBubble {
    enum class Mode { OFF, SESSION, RINGING }

    /** Pulsar (`true`) o soltar la burbuja en una sala. */
    var onPtt: ((Boolean) -> Unit)? = null

    private var context: Context? = null
    private var mode = Mode.OFF
    private var talking = false
    private var paused = false
    private var peerTalking = false
    private var caller = ""
    private var enabled = true
    private var appVisible = true

    /** Quitada a mano: no vuelve hasta salir otra vez de la app. */
    private var dismissed = false

    private var bubble: BubbleView? = null
    private var params: WindowManager.LayoutParams? = null
    private var target: DismissTarget? = null
    private var snap: ValueAnimator? = null

    /** Estado que manda Dart cada vez que cambia algo de la sala. */
    fun update(
        context: Context,
        mode: Mode,
        talking: Boolean,
        paused: Boolean,
        peerTalking: Boolean,
        caller: String,
        enabled: Boolean,
    ) {
        this.context = context.applicationContext
        // Una sala o una llamada nueva vuelve a sacar la burbuja quitada.
        if (mode != this.mode && mode != Mode.OFF) dismissed = false
        this.mode = mode
        this.talking = talking
        this.paused = paused
        this.peerTalking = peerTalking
        this.caller = caller
        this.enabled = enabled
        refresh()
    }

    /** Lo llama la actividad al verse u ocultarse. */
    fun setAppVisible(context: Context, visible: Boolean) {
        this.context = context.applicationContext
        appVisible = visible
        if (visible) dismissed = false
        refresh()
    }

    /** `false` mientras falte el permiso de mostrar encima de otras apps. */
    fun allowed(context: Context): Boolean = Settings.canDrawOverlays(context)

    /** Sin actividad no hay sala: la burbuja se va. */
    fun release() {
        mode = Mode.OFF
        remove()
        onPtt = null
    }

    private fun refresh() {
        val ctx = context ?: return
        val show = enabled && !appVisible && !dismissed && mode != Mode.OFF && allowed(ctx)
        if (!show) {
            remove()
            return
        }
        val view = bubble ?: add(ctx) ?: return
        view.render(mode, talking, paused, peerTalking, caller)
    }

    private fun windowType(): Int = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
        WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
    } else {
        @Suppress("DEPRECATION")
        WindowManager.LayoutParams.TYPE_PHONE
    }

    private fun windowManager(ctx: Context) = ctx.getSystemService(Context.WINDOW_SERVICE) as WindowManager

    private fun add(ctx: Context): BubbleView? {
        val view = BubbleView(ctx)
        val size = dp(ctx, BUBBLE_WINDOW_DP)
        val prefs = ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val metrics = ctx.resources.displayMetrics
        val lp = WindowManager.LayoutParams(
            size,
            size,
            windowType(),
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE,
            PixelFormat.TRANSLUCENT,
        ).apply {
            gravity = Gravity.TOP or Gravity.START
            // Por defecto a la derecha, a un tercio de la altura: lejos de la
            // barra de estado y de los botones de abajo de casi todas las apps.
            x = prefs.getInt(KEY_X, metrics.widthPixels - size)
            y = prefs.getInt(KEY_Y, metrics.heightPixels / 3)
        }
        clamp(ctx, lp)
        // Puede fallar si el permiso se retiró justo ahora: sin burbuja, la
        // notificación sigue sirviendo.
        if (runCatching { windowManager(ctx).addView(view, lp) }.isFailure) return null
        bubble = view
        params = lp
        return view
    }

    private fun remove() {
        val ctx = context ?: return
        snap?.cancel()
        snap = null
        hideTarget()
        bubble?.let { view ->
            // Desaparecer con el dedo encima no puede dejar el micrófono abierto.
            if (view.holding) onPtt?.invoke(false)
            view.stop()
            runCatching { windowManager(ctx).removeView(view) }
        }
        bubble = null
        params = null
    }

    private fun clamp(ctx: Context, lp: WindowManager.LayoutParams) {
        val metrics = ctx.resources.displayMetrics
        val size = dp(ctx, BUBBLE_WINDOW_DP)
        lp.x = lp.x.coerceIn(0, max(0, metrics.widthPixels - size))
        lp.y = lp.y.coerceIn(0, max(0, metrics.heightPixels - size))
    }

    private fun moveTo(x: Int, y: Int) {
        val ctx = context ?: return
        val view = bubble ?: return
        val lp = params ?: return
        lp.x = x
        lp.y = y
        clamp(ctx, lp)
        runCatching { windowManager(ctx).updateViewLayout(view, lp) }
        target?.highlight(overTarget(ctx, lp))
    }

    private fun overTarget(ctx: Context, lp: WindowManager.LayoutParams): Boolean {
        val metrics = ctx.resources.displayMetrics
        val half = dp(ctx, BUBBLE_WINDOW_DP) / 2f
        val (tx, ty) = targetCenter(ctx)
        val dx = lp.x + half - tx
        val dy = lp.y + half - ty
        return hypot(dx, dy) < max(dp(ctx, 72).toFloat(), metrics.widthPixels * 0.12f)
    }

    private fun targetCenter(ctx: Context): Pair<Float, Float> {
        val metrics = ctx.resources.displayMetrics
        return metrics.widthPixels / 2f to metrics.heightPixels - dp(ctx, 110).toFloat()
    }

    private fun showTarget(ctx: Context) {
        if (target != null) return
        val view = DismissTarget(ctx)
        val size = dp(ctx, TARGET_WINDOW_DP)
        val (cx, cy) = targetCenter(ctx)
        val lp = WindowManager.LayoutParams(
            size,
            size,
            windowType(),
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE,
            PixelFormat.TRANSLUCENT,
        ).apply {
            gravity = Gravity.TOP or Gravity.START
            x = (cx - size / 2f).toInt()
            y = (cy - size / 2f).toInt()
        }
        if (runCatching { windowManager(ctx).addView(view, lp) }.isSuccess) target = view
    }

    private fun hideTarget() {
        val ctx = context ?: return
        target?.let { runCatching { windowManager(ctx).removeView(it) } }
        target = null
    }

    private fun onDragStart() {
        val ctx = context ?: return
        snap?.cancel()
        showTarget(ctx)
    }

    /** Soltada tras arrastrar: a la equis la quita; si no, se pega al borde. */
    private fun onDragEnd() {
        val ctx = context ?: return
        val lp = params ?: return
        val drop = overTarget(ctx, lp)
        hideTarget()
        if (drop) {
            dismissed = true
            remove()
            return
        }
        val metrics = ctx.resources.displayMetrics
        val size = dp(ctx, BUBBLE_WINDOW_DP)
        val edge = if (lp.x + size / 2 < metrics.widthPixels / 2) 0 else metrics.widthPixels - size
        val y = lp.y
        snap = ValueAnimator.ofInt(lp.x, edge).apply {
            duration = 180
            interpolator = DecelerateInterpolator()
            addUpdateListener { moveTo(it.animatedValue as Int, y) }
            start()
        }
        ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit()
            .putInt(KEY_X, edge)
            .putInt(KEY_Y, y)
            .apply()
    }

    /** Toque corto: abre la app; si están llamando, además contesta. */
    private fun onTap() {
        val ctx = context ?: return
        val open = Intent(ctx, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
        if (mode == Mode.RINGING) {
            Ringer.stop()
            SessionService.cancelCall(ctx)
            open.putExtra(SessionService.EXTRA_ANSWER, true).putExtra(EXTRA_FROM_BUBBLE, true)
        }
        // Con el permiso de mostrar encima, Android deja abrir la actividad
        // desde segundo plano.
        runCatching { ctx.startActivity(open) }
    }

    private fun canTalk() = mode == Mode.SESSION && !paused

    private fun dp(ctx: Context, value: Int): Int = (value * ctx.resources.displayMetrics.density + 0.5f).toInt()

    /** Marca el intent de "Contestar" que viene de la burbuja. */
    const val EXTRA_FROM_BUBBLE = "dev.gravitaltalk.gravital_talk_app.FROM_BUBBLE"

    private const val PREFS = "ptt_bubble"
    private const val KEY_X = "x"
    private const val KEY_Y = "y"

    /** Ventana de la burbuja: el círculo más sitio para el pulso. */
    private const val BUBBLE_WINDOW_DP = 92
    private const val BUBBLE_RADIUS_DP = 30
    private const val TARGET_WINDOW_DP = 72

    /**
     * Lo que hay que mantener el dedo quieto para que cuente como hablar y no
     * como arrastrar. Por debajo, cada intento de mover la burbuja abría el
     * micrófono un instante (y sonaba el tono de turno en los dos lados).
     */
    private const val HOLD_DELAY_MS = 150L

    // Los de la paleta de la app (`GravitalPalette`).
    private const val COLOR_BRAND = 0xFF0D9488.toInt()
    private const val COLOR_TRANSMIT = 0xFFF5A623.toInt()
    private const val COLOR_ONLINE = 0xFF16A34A.toInt()
    private const val COLOR_PAUSED = 0xFF6B7280.toInt()
    private const val COLOR_DANGER = 0xFFDC2626.toInt()

    /** El círculo: color por estado, micrófono o teléfono, y pulso. */
    @SuppressLint("ViewConstructor")
    private class BubbleView(context: Context) : View(context) {
        var holding = false
            private set

        private var mode = Mode.SESSION
        private var talking = false
        private var paused = false
        private var peerTalking = false

        private var pressed = false
        private var dragging = false
        private var downX = 0f
        private var downY = 0f
        private var startX = 0
        private var startY = 0
        private val slop = ViewConfiguration.get(context).scaledTouchSlop

        private val density = context.resources.displayMetrics.density
        private val fill = Paint(Paint.ANTI_ALIAS_FLAG)
        private val ring = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.STROKE }
        private val icon = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.WHITE
            strokeCap = Paint.Cap.ROUND
        }
        private val rect = RectF()
        private var phase = 0f
        private var pulse: ValueAnimator? = null

        private val holdStart = Runnable {
            if (!canTalk() || dragging) return@Runnable
            holding = true
            performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
            onPtt?.invoke(true)
            invalidate()
        }

        init {
            // La sombra del círculo sólo se dibuja por software.
            setLayerType(LAYER_TYPE_SOFTWARE, null)
            contentDescription = "Mantén pulsado para hablar"
        }

        fun render(mode: Mode, talking: Boolean, paused: Boolean, peerTalking: Boolean, caller: String) {
            this.mode = mode
            this.talking = talking
            this.paused = paused
            this.peerTalking = peerTalking
            contentDescription = when {
                mode == Mode.RINGING -> "$caller quiere hablar. Toca para contestar"
                paused -> "Sala en pausa. Toca para volver a la app"
                else -> "Mantén pulsado para hablar"
            }
            val wantsPulse = mode == Mode.RINGING || (peerTalking && !talking && !paused)
            if (wantsPulse && pulse == null) {
                pulse = ValueAnimator.ofFloat(0f, 1f).apply {
                    duration = if (mode == Mode.RINGING) 900 else 1200
                    repeatCount = ValueAnimator.INFINITE
                    addUpdateListener {
                        phase = it.animatedValue as Float
                        invalidate()
                    }
                    start()
                }
            } else if (!wantsPulse) {
                pulse?.cancel()
                pulse = null
                phase = 0f
            }
            invalidate()
        }

        fun stop() {
            removeCallbacks(holdStart)
            pulse?.cancel()
            pulse = null
        }

        @SuppressLint("ClickableViewAccessibility")
        override fun onTouchEvent(event: MotionEvent): Boolean {
            val lp = params ?: return false
            when (event.actionMasked) {
                MotionEvent.ACTION_DOWN -> {
                    downX = event.rawX
                    downY = event.rawY
                    startX = lp.x
                    startY = lp.y
                    dragging = false
                    holding = false
                    pressed = true
                    if (canTalk()) postDelayed(holdStart, HOLD_DELAY_MS)
                    invalidate()
                }
                MotionEvent.ACTION_MOVE -> {
                    // Hablando, el dedo puede bailar: no se mueve ni se corta.
                    if (holding) return true
                    val dx = event.rawX - downX
                    val dy = event.rawY - downY
                    if (!dragging && hypot(dx, dy) > slop) {
                        dragging = true
                        removeCallbacks(holdStart)
                        onDragStart()
                    }
                    if (dragging) moveTo(startX + dx.toInt(), startY + dy.toInt())
                }
                MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                    removeCallbacks(holdStart)
                    pressed = false
                    val cancelled = event.actionMasked == MotionEvent.ACTION_CANCEL
                    when {
                        holding -> {
                            holding = false
                            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O_MR1) {
                                performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY_RELEASE)
                            }
                            onPtt?.invoke(false)
                        }
                        dragging -> onDragEnd()
                        !cancelled -> {
                            performClick()
                            onTap()
                        }
                    }
                    dragging = false
                    invalidate()
                }
            }
            return true
        }

        override fun performClick(): Boolean {
            super.performClick()
            return true
        }

        override fun onDraw(canvas: Canvas) {
            val cx = width / 2f
            val cy = height / 2f
            val grow = if (pressed || holding || talking) 1.1f else 1f
            val r = BUBBLE_RADIUS_DP * density * grow

            // Pulso: el otro está hablando, o la llamada espera respuesta.
            if (pulse != null) {
                ring.color = COLOR_ONLINE
                ring.strokeWidth = 3 * density
                ring.alpha = ((1f - phase) * 200).toInt()
                canvas.drawCircle(cx, cy, r + phase * 12 * density, ring)
            }

            fill.color = when {
                mode == Mode.RINGING -> COLOR_ONLINE
                paused -> COLOR_PAUSED
                talking || holding -> COLOR_TRANSMIT
                else -> COLOR_BRAND
            }
            fill.alpha = if (paused) 210 else 245
            fill.setShadowLayer(4 * density, 0f, 2 * density, 0x66000000)
            canvas.drawCircle(cx, cy, r, fill)
            fill.clearShadowLayer()

            // Mientras se habla, un aro blanco: se ve aunque el color de la
            // app de debajo se parezca al ámbar.
            if (talking) {
                ring.color = Color.WHITE
                ring.alpha = 230
                ring.strokeWidth = 2.5f * density
                canvas.drawCircle(cx, cy, r - 4 * density, ring)
            }

            if (mode == Mode.RINGING) drawPhone(canvas, cx, cy, r) else drawMic(canvas, cx, cy, r)
        }

        private fun drawMic(canvas: Canvas, cx: Float, cy: Float, r: Float) {
            val w = r * 0.36f
            val h = r * 0.62f
            val top = cy - r * 0.48f
            icon.style = Paint.Style.FILL
            rect.set(cx - w / 2, top, cx + w / 2, top + h)
            canvas.drawRoundRect(rect, w / 2, w / 2, icon)
            icon.style = Paint.Style.STROKE
            icon.strokeWidth = r * 0.09f
            val arc = r * 0.34f
            rect.set(cx - arc, top + h * 0.35f, cx + arc, top + h * 0.35f + arc * 2)
            canvas.drawArc(rect, 0f, 180f, false, icon)
            val stemTop = top + h * 0.35f + arc * 2
            canvas.drawLine(cx, stemTop, cx, stemTop + r * 0.16f, icon)
            canvas.drawLine(cx - r * 0.2f, stemTop + r * 0.16f, cx + r * 0.2f, stemTop + r * 0.16f, icon)
            if (paused) {
                // Tachado: en pausa no se puede hablar.
                canvas.drawLine(cx - r * 0.5f, cy - r * 0.5f, cx + r * 0.5f, cy + r * 0.5f, icon)
            }
        }

        private fun drawPhone(canvas: Canvas, cx: Float, cy: Float, r: Float) {
            val drawable = context.getDrawable(android.R.drawable.stat_sys_phone_call)?.mutate() ?: return
            drawable.setTint(Color.WHITE)
            val half = (r * 0.55f).toInt()
            drawable.setBounds(cx.toInt() - half, cy.toInt() - half, cx.toInt() + half, cy.toInt() + half)
            drawable.draw(canvas)
        }
    }

    /** La equis de abajo sobre la que se suelta la burbuja para quitarla. */
    @SuppressLint("ViewConstructor")
    private class DismissTarget(context: Context) : View(context) {
        private val density = context.resources.displayMetrics.density
        private val fill = Paint(Paint.ANTI_ALIAS_FLAG)
        private val cross = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.WHITE
            style = Paint.Style.STROKE
            strokeCap = Paint.Cap.ROUND
            strokeWidth = 3 * context.resources.displayMetrics.density
        }
        private var over = false

        fun highlight(on: Boolean) {
            if (on == over) return
            over = on
            if (on) performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
            invalidate()
        }

        override fun onDraw(canvas: Canvas) {
            val cx = width / 2f
            val cy = height / 2f
            val r = min(width, height) / 2f * if (over) 0.95f else 0.75f
            fill.color = if (over) COLOR_DANGER else 0xCC1F2937.toInt()
            canvas.drawCircle(cx, cy, r, fill)
            val arm = 9 * density
            canvas.drawLine(cx - arm, cy - arm, cx + arm, cy + arm, cross)
            canvas.drawLine(cx + arm, cy - arm, cx - arm, cy + arm, cross)
        }
    }
}
