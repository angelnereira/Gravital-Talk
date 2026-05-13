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
import kotlinx.coroutines.withTimeoutOrNull
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

            // 3. Descubrir IP pública via STUN (bloqueante ~5 s).
            _uiState.value = _uiState.value.copy(statusMsg = appContext.getString(R.string.pairing_discovering_ip))
            val publicAddr = GravitalTalkJni.nativeDiscoverPublicAddr(localPort)

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

            // 6. Esperar cliente (bloqueante). nativeClose() en otro hilo lo interrumpe.
            val status = GravitalTalkJni.nativeAcceptAny(handle)

            // Si hangUp() tomó el ownership mientras esperábamos, no tocamos nada.
            if (nativeHandle != handle) return@launch

            if (status != 0) {
                nativeHandle = 0L
                runCatching { GravitalTalkJni.nativeDestroy(handle) }
                _uiState.value = _uiState.value.copy(
                    screen = PairingScreen.Home,
                    statusMsg = "",
                    error = if (status == -7) null else "Handshake failed ($status)",
                )
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

    private suspend fun attemptConnect(lan: String?, pub: String?, relay: String?) {
        val handle = GravitalTalkJni.nativeCreate(48_000, 1, 0)
        if (handle == 0L) {
            _uiState.value = _uiState.value.copy(error = "Failed to create session")
            return
        }
        nativeHandle = handle

        // Intentar LAN (2 s) → IP pública (5 s) → relay (sin timeout adicional).
        val candidates = buildList {
            if (lan != null) add(Pair(lan, 2_000L))
            if (pub != null) add(Pair(pub, 5_000L))
            if (relay != null) add(Pair(relay, 10_000L))
        }

        for ((addr, timeoutMs) in candidates) {
            val (host, port) = parseHostPort(addr) ?: continue
            _uiState.value = _uiState.value.copy(
                statusMsg = appContext.getString(
                    if (timeoutMs <= 2_000L) R.string.pairing_connecting_direct
                    else R.string.pairing_connecting_relay
                )
            )
            val ok = withTimeoutOrNull(timeoutMs) {
                withContext(Dispatchers.IO) {
                    GravitalTalkJni.nativeConnect(handle, host, port) == 0
                }
            } ?: false

            if (ok) {
                _uiState.value = _uiState.value.copy(
                    screen = PairingScreen.Connected(handle),
                    statusMsg = "",
                )
                return
            }
        }

        // Todos los intentos fallaron.
        GravitalTalkJni.nativeDestroy(handle)
        nativeHandle = 0L
        _uiState.value = _uiState.value.copy(
            error = "Could not connect to peer",
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
