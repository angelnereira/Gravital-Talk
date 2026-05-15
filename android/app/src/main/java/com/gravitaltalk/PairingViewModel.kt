package com.gravitaltalk

import android.content.Context
import android.graphics.Bitmap
import android.net.ConnectivityManager
import android.net.LinkProperties
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.net.Inet4Address
import java.net.URI

// ── Estados de pantalla ────────────────────────────────────────────────────────

sealed class PairingScreen {
    object Home : PairingScreen()
    object Hosting : PairingScreen()
    object Joining : PairingScreen()
    data class Connected(val handle: Long) : PairingScreen()
}

data class PairingUiState(
    val screen: PairingScreen = PairingScreen.Home,
    val qrBitmap: Bitmap? = null,
    val textCode: String = "",
    val statusMsg: String = "",
    val error: String? = null,
)

class PairingViewModel(private val appContext: Context) : ViewModel() {

    private val _uiState = MutableStateFlow(PairingUiState())
    val uiState: StateFlow<PairingUiState> = _uiState.asStateFlow()

    // Acceso sincronizado sólo desde el hilo main (StateFlow/ViewModel).
    // Antes de destruir un handle siempre se pone a 0 para evitar doble-free.
    @Volatile private var nativeHandle: Long = 0L
    private var hostJob: Job? = null

    // ── Modo HOST ──────────────────────────────────────────────────────────────

    /**
     * Empieza a hospedar: descubre IPs, genera QR y espera cualquier cliente.
     *
     * @param relayUrl URL del relay opcional, p.ej. "relay.example.com:9000".
     *                 Incluido en el QR como fallback para el cliente.
     */
    fun startHosting(relayUrl: String? = null) {
        hostJob?.cancel()
        hostJob = viewModelScope.launch(Dispatchers.IO) {
            _uiState.value = PairingUiState(
                screen = PairingScreen.Hosting,
                statusMsg = appContext.getString(R.string.pairing_preparing),
            )

            // 1. Crear sesión nativa con puerto efímero.
            val handle = GravitalTalkJni.nativeCreate(48_000, 1, 0)
            if (handle == 0L) {
                _uiState.value = _uiState.value.copy(
                    error = "Failed to create native session",
                )
                return@launch
            }
            nativeHandle = handle

            val localPort = GravitalTalkJni.nativeGetLocalPort(handle)
            if (localPort <= 0) {
                _uiState.value = _uiState.value.copy(error = "Could not get local port")
                return@launch
            }

            // 2. Obtener IP LAN (primer IPv4 de la interfaz activa).
            val lanIp = getLanAddress()

            // 3. Descubrir IP pública via STUN usando puerto efímero (0) para evitar
            //    conflicto con el socket de sesión ya bindeado en localPort.
            //    Combinamos la IP pública que devuelve STUN con el puerto local de sesión.
            _uiState.value = _uiState.value.copy(statusMsg = appContext.getString(R.string.pairing_discovering_ip))
            val stunResult = GravitalTalkJni.nativeDiscoverPublicAddr(0)
            // stunResult tiene formato "ip:puerto_efimero" — sólo nos interesa la IP.
            val publicAddr = stunResult?.let { result ->
                val stunIp = result.substringBeforeLast(":")
                if (stunIp.isNotBlank()) "$stunIp:$localPort" else null
            }

            // 4. Armar URI del QR.
            val uriBuilder = StringBuilder("gravital-talk://pair?v=1")
            if (lanIp != null) uriBuilder.append("&lan=$lanIp:$localPort")
            if (publicAddr != null) uriBuilder.append("&pub=$publicAddr")
            if (!relayUrl.isNullOrBlank()) uriBuilder.append("&relay=$relayUrl")
            val pairUri = uriBuilder.toString()

            // Código de texto corto: 8 chars hex del handle (suficientemente único).
            val textCode = "GRVT-" + handle.toUInt().toString(16).uppercase().takeLast(4).padStart(4, '0')

            // 5. Generar QR bitmap.
            val qrBitmap = generateQrBitmap(pairUri, 512)

            _uiState.value = _uiState.value.copy(
                qrBitmap = qrBitmap,
                textCode = textCode,
                statusMsg = appContext.getString(R.string.pairing_waiting),
            )

            // 6. Esperar cliente. El loop en Rust reintenta hasta que close() sea llamado.
            //    nativeClose (desde hangUp) activa el flag closed → sale en ≤500 ms.
            val status = GravitalTalkJni.nativeAcceptAny(handle)

            // Si hangUp() tomó el ownership mientras esperábamos, no tocamos nada.
            if (nativeHandle != handle) return@launch

            if (status != 0) {
                nativeHandle = 0L
                runCatching { GravitalTalkJni.nativeDestroy(handle) }
                // -8 = GS_ERR_CLOSED (close() llamado intencionalmente) → no mostrar error
                if (status != -8) {
                    _uiState.value = _uiState.value.copy(
                        screen = PairingScreen.Home,
                        statusMsg = "",
                        error = null,
                    )
                }
            } else {
                _uiState.value = _uiState.value.copy(
                    screen = PairingScreen.Connected(handle),
                    statusMsg = "",
                )
            }
        }
    }

    // ── Modo JOIN ──────────────────────────────────────────────────────────────

    /**
     * Parsea un URI de pairing QR e intenta conectarse en orden:
     * LAN (2 s) → IP pública (5 s) → relay (10 s).
     */
    fun joinFromQr(qrData: String) {
        viewModelScope.launch(Dispatchers.IO) {
            _uiState.value = _uiState.value.copy(
                screen = PairingScreen.Joining,
                statusMsg = appContext.getString(R.string.pairing_connecting_direct),
            )
            val uri = runCatching { URI(qrData) }.getOrNull()
            if (uri == null || uri.scheme != "gravital-talk") {
                _uiState.value = _uiState.value.copy(error = "Invalid QR code")
                return@launch
            }

            val params = uri.query?.split("&")
                ?.associate { it.substringBefore("=") to it.substringAfter("=") }
                ?: emptyMap()

            val lan = params["lan"]
            val pub = params["pub"]
            val relay = params["relay"]

            attemptConnect(lan, pub, relay)
        }
    }

    /**
     * Conecta manualmente usando host:port del relay para signaling.
     * El cliente hace `nativeConnect` directo al relay.
     */
    fun joinFromRelay(relayHost: String, relayPort: Int) {
        viewModelScope.launch(Dispatchers.IO) {
            _uiState.value = _uiState.value.copy(
                screen = PairingScreen.Joining,
                statusMsg = appContext.getString(R.string.pairing_connecting_relay),
            )
            attemptConnect(null, null, "$relayHost:$relayPort")
        }
    }

    /**
     * Conecta a una "red" (relay) sin QR. El usuario introduce host:port directamente.
     * Soporta formato "host:port" o "host" (usa 9000 como puerto por defecto).
     */
    fun joinFromRoom(addr: String) {
        val lastColon = addr.lastIndexOf(':')
        val host: String
        val port: Int
        if (lastColon > 0) {
            host = addr.substring(0, lastColon)
            port = addr.substring(lastColon + 1).toIntOrNull() ?: 9000
        } else {
            host = addr
            port = 9000
        }
        joinFromRelay(host, port)
    }

    // ── Colgar ─────────────────────────────────────────────────────────────────

    fun hangUp() {
        // Tomar ownership PRIMERO antes de cancelar el job para evitar doble-free.
        val h = nativeHandle
        nativeHandle = 0L
        hostJob?.cancel()
        hostJob = null
        if (h != 0L) {
            // nativeClose interrumpe nativeAcceptAny en el hilo IO, luego destroy.
            viewModelScope.launch(Dispatchers.IO) {
                runCatching { GravitalTalkJni.nativeClose(h) }
                runCatching { GravitalTalkJni.nativeDestroy(h) }
            }
        }
        _uiState.value = PairingUiState(screen = PairingScreen.Home)
    }

    // ── Lifecycle ──────────────────────────────────────────────────────────────

    override fun onCleared() {
        super.onCleared()
        val h = nativeHandle
        nativeHandle = 0L
        if (h != 0L) {
            runCatching { GravitalTalkJni.nativeClose(h) }
            runCatching { GravitalTalkJni.nativeDestroy(h) }
        }
    }

    // ── Helpers privados ───────────────────────────────────────────────────────

    /**
     * Intenta conectar a los candidatos en orden: LAN → IP pública → relay.
     *
     * Cada candidato usa un handle NUEVO e independiente. nativeConnect es
     * una llamada JNI bloqueante — nunca se llama dos veces sobre el mismo
     * handle para evitar acceso concurrente a la misma sesión nativa.
     * El timeout por intento lo gestiona Rust (HANDSHAKE_TIMEOUT_MS = 10 s).
     */
    private suspend fun attemptConnect(lan: String?, pub: String?, relay: String?) {
        val candidates = buildList {
            if (lan != null) add(Pair(lan, R.string.pairing_connecting_direct))
            if (pub != null) add(Pair(pub, R.string.pairing_connecting_direct))
            if (relay != null) add(Pair(relay, R.string.pairing_connecting_relay))
        }

        if (candidates.isEmpty()) {
            _uiState.value = _uiState.value.copy(
                error = "QR no contiene dirección de conexión",
                screen = PairingScreen.Joining,
            )
            return
        }

        for ((addr, statusRes) in candidates) {
            val (host, port) = parseHostPort(addr) ?: continue

            // Crear handle fresco para este intento.
            val h = GravitalTalkJni.nativeCreate(48_000, 1, 0)
            if (h == 0L) continue
            nativeHandle = h

            _uiState.value = _uiState.value.copy(
                statusMsg = appContext.getString(statusRes)
            )

            // nativeConnect bloquea hasta completar o timeout Rust (10 s).
            // NO envolver con withTimeoutOrNull: cancelar Kotlin no cancela JNI.
            val status = GravitalTalkJni.nativeConnect(h, host, port)

            // Verificar que nadie más tomó el handle (hangUp).
            if (nativeHandle != h) return

            if (status == 0) {
                _uiState.value = _uiState.value.copy(
                    screen = PairingScreen.Connected(h),
                    statusMsg = "",
                )
                return
            }

            // Fallido: liberar este handle y probar el siguiente.
            nativeHandle = 0L
            runCatching { GravitalTalkJni.nativeDestroy(h) }
        }

        // Todos los candidatos fallaron.
        _uiState.value = _uiState.value.copy(
            error = "No se pudo conectar. Asegúrate de estar en la misma red.",
            screen = PairingScreen.Joining,
        )
    }

    private fun parseHostPort(addr: String): Pair<String, Int>? {
        val lastColon = addr.lastIndexOf(':')
        if (lastColon < 0) return null
        val host = addr.substring(0, lastColon)
        val port = addr.substring(lastColon + 1).toIntOrNull() ?: return null
        return Pair(host, port)
    }

    private fun getLanAddress(): String? {
        val cm = appContext.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        val network = cm.activeNetwork ?: return null
        val props: LinkProperties = cm.getLinkProperties(network) ?: return null
        return props.linkAddresses
            .mapNotNull { it.address as? Inet4Address }
            .firstOrNull { !it.isLoopbackAddress }
            ?.hostAddress
    }

    private fun generateQrBitmap(content: String, size: Int): Bitmap? = runCatching {
        val writer = QRCodeWriter()
        val hints = mapOf(EncodeHintType.MARGIN to 1)
        val matrix = writer.encode(content, BarcodeFormat.QR_CODE, size, size, hints)
        val bmp = Bitmap.createBitmap(size, size, Bitmap.Config.RGB_565)
        for (x in 0 until size) {
            for (y in 0 until size) {
                bmp.setPixel(x, y, if (matrix[x, y]) 0xFF000000.toInt() else 0xFFFFFFFF.toInt())
            }
        }
        bmp
    }.getOrNull()
}

class PairingViewModelFactory(private val context: Context) : ViewModelProvider.Factory {
    override fun <T : ViewModel> create(modelClass: Class<T>): T {
        @Suppress("UNCHECKED_CAST")
        return PairingViewModel(context.applicationContext) as T
    }
}
