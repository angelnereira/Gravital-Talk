package com.gravitaltalk

/**
 * Puente JNI a la librería nativa de Gravital Talk.
 *
 * La .so (`libgravital_talk_ffi.so`) debe estar en `src/main/jniLibs/<abi>/`
 * antes de compilar. Generarla con:
 *
 *   make android-libs
 *
 * Todos los `handle` son punteros opacos (Long) al objeto `SessionInner`
 * en el heap nativo. No compartir entre hilos sin sincronización externa.
 *
 * Códigos de retorno: 0 = GS_OK, negativo = error (ver GsStatus en Rust).
 */
object GravitalTalkJni {

    init {
        System.loadLibrary("gravital_talk_ffi")
    }

    // ── Info ──────────────────────────────────────────────────────────────────

    /** Versión del crate Rust. */
    external fun nativeVersion(): String

    // ── Ciclo de vida ─────────────────────────────────────────────────────────

    /**
     * Crea una sesión UDP.
     *
     * @param sampleRate  Sample rate en Hz (p.ej. 48000).
     * @param channels    Número de canales (1 = mono).
     * @param bindPort    Puerto local UDP (0 = efímero).
     * @return Handle de sesión (≠0), o 0 en error.
     */
    external fun nativeCreate(sampleRate: Int, channels: Int, bindPort: Int): Long

    /** Destruye la sesión y libera recursos nativos. El handle queda inválido. */
    external fun nativeDestroy(handle: Long)

    // ── Handshake ─────────────────────────────────────────────────────────────

    /**
     * Handshake como cliente hacia `host:port`. Bloqueante.
     * @return GsStatus (0 = OK).
     */
    external fun nativeConnect(handle: Long, host: String, port: Int): Int

    /**
     * Handshake como servidor esperando desde `peerHost:peerPort`. Bloqueante.
     * @return GsStatus (0 = OK).
     */
    external fun nativeAccept(handle: Long, peerHost: String, peerPort: Int): Int

    // ── Audio ─────────────────────────────────────────────────────────────────

    /**
     * Envía un frame de audio PCM 16-bit LE, mono 48 kHz (20 ms = 1920 bytes).
     * @return GsStatus.
     */
    external fun nativeSendAudio(handle: Long, pcm: ByteArray): Int

    /**
     * Recibe el próximo frame de audio PCM. Bloqueante.
     * @return ByteArray de PCM, o null en error/cierre.
     */
    external fun nativeRecvAudio(handle: Long): ByteArray?

    // ── PTT (Floor Control) ───────────────────────────────────────────────────

    /** Solicita el floor (inicio de transmisión). @return GsStatus. */
    external fun nativePttPress(handle: Long): Int

    /** Libera el floor (fin de transmisión). @return GsStatus. */
    external fun nativePttRelease(handle: Long): Int

    /**
     * Comprueba si el peer remoto está transmitiendo.
     * @return 1 si activo, 0 si no.
     */
    external fun nativeIsPeerPttActive(handle: Long): Int

    // ── Métricas ──────────────────────────────────────────────────────────────

    /**
     * Obtiene métricas de la sesión.
     * @return FloatArray [rtt_ms, jitter_ms, loss_percent, estimated_mos].
     */
    external fun nativeGetMetrics(handle: Long): FloatArray

    /** Devuelve el session_id negociado. */
    external fun nativeGetSessionId(handle: Long): Int

    // ── Cierre ────────────────────────────────────────────────────────────────

    /** Cierra la sesión enviando el paquete CLOSE al peer. @return GsStatus. */
    external fun nativeClose(handle: Long): Int

    // ── Pairing (QR / código) ─────────────────────────────────────────────────

    /**
     * Devuelve el puerto UDP local del socket (>0), o 0 en error.
     * Necesario para armar el QR de pairing.
     */
    external fun nativeGetLocalPort(handle: Long): Int

    /**
     * Descubre la IP pública via STUN (stun.l.google.com:19302).
     * Bloqueante ~5 s. Retorna "ip:port" o null si todos los servidores fallan.
     *
     * @param bindPort Puerto local (0 = efímero). Usar el mismo que la sesión.
     */
    external fun nativeDiscoverPublicAddr(bindPort: Int): String?

    /**
     * Handshake servidor sin conocer la IP del cliente de antemano.
     * Acepta el primer cliente que llegue (modo QR pairing). Bloqueante.
     * @return GsStatus (0 = OK).
     */
    external fun nativeAcceptAny(handle: Long): Int
}
