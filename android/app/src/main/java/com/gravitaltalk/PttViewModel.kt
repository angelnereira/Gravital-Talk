package com.gravitaltalk

import android.content.Context
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioRecord
import android.media.AudioTrack
import android.media.MediaRecorder
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.os.PowerManager
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlin.math.PI
import kotlin.math.sin

data class PttMetrics(
    val rttMs: Float = 0f,
    val jitterMs: Float = 0f,
    val lossPercent: Float = 0f,
    val estimatedMos: Float = 4.5f,
)

sealed class PttConnectionState {
    object Idle : PttConnectionState()
    object Connecting : PttConnectionState()
    object Reconnecting : PttConnectionState()
    data class Connected(val sessionId: Int) : PttConnectionState()
    data class Error(val message: String) : PttConnectionState()
}

class PttViewModel(private val appContext: Context) : ViewModel() {

    companion object {
        private const val SAMPLE_RATE = 48_000
        private const val CHANNELS = 1
        private const val FRAME_DURATION_MS = 20
        private const val FRAME_SAMPLES = SAMPLE_RATE * FRAME_DURATION_MS / 1000  // 960
        private const val FRAME_BYTES = FRAME_SAMPLES * 2                           // 1920 (16-bit)
        private const val TONE_PRESS_FREQ_HZ = 880.0
        private const val TONE_RELEASE_FREQ_HZ = 440.0
        private const val TONE_PRESS_MS = 100
        private const val TONE_RELEASE_MS = 80
    }

    private val _connectionState = MutableStateFlow<PttConnectionState>(PttConnectionState.Idle)
    val connectionState: StateFlow<PttConnectionState> = _connectionState.asStateFlow()

    private val _isPttActive = MutableStateFlow(false)
    val isPttActive: StateFlow<Boolean> = _isPttActive.asStateFlow()

    private val _isPeerPttActive = MutableStateFlow(false)
    val isPeerPttActive: StateFlow<Boolean> = _isPeerPttActive.asStateFlow()

    private val _metrics = MutableStateFlow(PttMetrics())
    val metrics: StateFlow<PttMetrics> = _metrics.asStateFlow()

    private var nativeHandle: Long = 0L
    private var captureJob: Job? = null
    private var playbackJob: Job? = null
    private var metricsJob: Job? = null
    private var peerMonitorJob: Job? = null

    // Connection parameters (saved for reconnect).
    private var lastRelayHost: String = ""
    private var lastRelayPort: Int = 9000

    // WakeLock — prevents CPU sleep during active PTT call.
    private val wakeLock: PowerManager.WakeLock by lazy {
        val pm = appContext.getSystemService(Context.POWER_SERVICE) as PowerManager
        pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "GravitalTalk:PttWakeLock")
    }

    // NetworkCallback — triggers reconnect when network changes.
    private val networkCallback = object : ConnectivityManager.NetworkCallback() {
        override fun onAvailable(network: Network) {
            val state = _connectionState.value
            if (state is PttConnectionState.Error || state is PttConnectionState.Reconnecting) {
                viewModelScope.launch { reconnect() }
            }
        }
        override fun onLost(network: Network) {
            val state = _connectionState.value
            if (state is PttConnectionState.Connected) {
                viewModelScope.launch(Dispatchers.IO) {
                    _connectionState.value = PttConnectionState.Reconnecting
                    stopAllTasks()
                    destroyNative()
                }
            }
        }
    }

    init {
        registerNetworkCallback()
    }

    private fun registerNetworkCallback() {
        val cm = appContext.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        val request = NetworkRequest.Builder()
            .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .build()
        runCatching { cm.registerNetworkCallback(request, networkCallback) }
    }

    // ─── Conexión ─────────────────────────────────────────────────────────────

    fun connect(relayHost: String, relayPort: Int = 9000) {
        val state = _connectionState.value
        if (state !is PttConnectionState.Idle && state !is PttConnectionState.Error) return

        lastRelayHost = relayHost
        lastRelayPort = relayPort
        viewModelScope.launch(Dispatchers.IO) {
            doConnect(relayHost, relayPort)
        }
    }

    private suspend fun reconnect() {
        if (lastRelayHost.isEmpty()) return
        _connectionState.value = PttConnectionState.Reconnecting
        var delayMs = 2_000L
        repeat(10) { attempt ->
            delay(delayMs)
            delayMs = (delayMs * 2).coerceAtMost(30_000L)
            val result = runCatching { doConnect(lastRelayHost, lastRelayPort) }
            if (_connectionState.value is PttConnectionState.Connected) return
        }
    }

    private suspend fun doConnect(relayHost: String, relayPort: Int) {
        _connectionState.value = PttConnectionState.Connecting

        val handle = GravitalTalkJni.nativeCreate(SAMPLE_RATE, CHANNELS, 0)
        if (handle == 0L) {
            _connectionState.value = PttConnectionState.Error("Failed to create session")
            return
        }
        nativeHandle = handle

        val status = GravitalTalkJni.nativeConnect(handle, relayHost, relayPort)
        if (status != 0) {
            GravitalTalkJni.nativeDestroy(handle)
            nativeHandle = 0L
            _connectionState.value = PttConnectionState.Error("Handshake failed: $status")
            return
        }

        val sessionId = GravitalTalkJni.nativeGetSessionId(handle)
        _connectionState.value = PttConnectionState.Connected(sessionId)

        startPlayback()
        startMetricsPolling()
        startPeerMonitor()
    }

    /**
     * Toma posesión de un handle nativo ya conectado (creado por PairingViewModel).
     * Úsalo cuando MainActivity recibe `EXTRA_NATIVE_HANDLE` de PairingActivity.
     */
    fun attachExistingHandle(handle: Long) {
        if (handle == 0L) return
        viewModelScope.launch(Dispatchers.IO) {
            nativeHandle = handle
            val sessionId = GravitalTalkJni.nativeGetSessionId(handle)
            _connectionState.value = PttConnectionState.Connected(sessionId)
            startPlayback()
            startMetricsPolling()
            startPeerMonitor()
        }
    }

    fun disconnect() {
        viewModelScope.launch(Dispatchers.IO) {
            stopAllTasks()
            destroyNative()
            _isPttActive.value = false
            _isPeerPttActive.value = false
            _connectionState.value = PttConnectionState.Idle
            releaseWakeLock()
        }
    }

    // ─── PTT ──────────────────────────────────────────────────────────────────

    fun pttPress() {
        val h = nativeHandle
        if (h == 0L || _connectionState.value !is PttConnectionState.Connected) return
        viewModelScope.launch(Dispatchers.IO) {
            acquireWakeLock()
            GravitalTalkJni.nativePttPress(h)
            _isPttActive.value = true
            playTone(TONE_PRESS_FREQ_HZ, TONE_PRESS_MS)
            startCapture()
        }
    }

    fun pttRelease() {
        val h = nativeHandle
        if (h == 0L) return
        viewModelScope.launch(Dispatchers.IO) {
            stopCapture()
            GravitalTalkJni.nativePttRelease(h)
            _isPttActive.value = false
            playTone(TONE_RELEASE_FREQ_HZ, TONE_RELEASE_MS)
            releaseWakeLock()
        }
    }

    // ─── Wake lock ────────────────────────────────────────────────────────────

    private fun acquireWakeLock() {
        if (!wakeLock.isHeld) {
            wakeLock.acquire(10 * 60 * 1000L) // max 10 min
        }
    }

    private fun releaseWakeLock() {
        if (wakeLock.isHeld) wakeLock.release()
    }

    // ─── PTT tones ────────────────────────────────────────────────────────────

    private fun playTone(freqHz: Double, durationMs: Int) {
        val nSamples = SAMPLE_RATE * durationMs / 1000
        val pcm = ShortArray(nSamples) { i ->
            val phase = 2.0 * PI * freqHz * i / SAMPLE_RATE
            (sin(phase) * 20_000.0).toInt().toShort()
        }
        val track = AudioTrack(
            AudioManager.STREAM_VOICE_CALL,
            SAMPLE_RATE,
            AudioFormat.CHANNEL_OUT_MONO,
            AudioFormat.ENCODING_PCM_16BIT,
            pcm.size * 2,
            AudioTrack.MODE_STATIC,
        )
        track.write(pcm, 0, pcm.size)
        track.play()
        // Release after playback (non-blocking — static mode plays once).
        viewModelScope.launch(Dispatchers.IO) {
            delay(durationMs.toLong() + 50)
            track.stop()
            track.release()
        }
    }

    // ─── Audio capture ────────────────────────────────────────────────────────

    private fun startCapture() {
        if (captureJob?.isActive == true) return
        captureJob = viewModelScope.launch(Dispatchers.IO) {
            val minBuf = AudioRecord.getMinBufferSize(
                SAMPLE_RATE,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT
            )
            val bufSize = maxOf(minBuf, FRAME_BYTES * 4)
            val recorder = AudioRecord(
                MediaRecorder.AudioSource.VOICE_COMMUNICATION,
                SAMPLE_RATE,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT,
                bufSize
            )
            recorder.startRecording()
            val frame = ByteArray(FRAME_BYTES)
            try {
                while (_isPttActive.value && nativeHandle != 0L) {
                    var offset = 0
                    while (offset < FRAME_BYTES) {
                        val read = recorder.read(frame, offset, FRAME_BYTES - offset)
                        if (read <= 0) break
                        offset += read
                    }
                    if (offset == FRAME_BYTES && nativeHandle != 0L) {
                        GravitalTalkJni.nativeSendAudio(nativeHandle, frame)
                    }
                }
            } finally {
                recorder.stop()
                recorder.release()
            }
        }
    }

    private fun stopCapture() {
        captureJob?.cancel()
        captureJob = null
    }

    // ─── Playback ─────────────────────────────────────────────────────────────

    private fun startPlayback() {
        playbackJob = viewModelScope.launch(Dispatchers.IO) {
            val minBuf = AudioTrack.getMinBufferSize(
                SAMPLE_RATE,
                AudioFormat.CHANNEL_OUT_MONO,
                AudioFormat.ENCODING_PCM_16BIT
            )
            val track = AudioTrack(
                AudioManager.STREAM_VOICE_CALL,
                SAMPLE_RATE,
                AudioFormat.CHANNEL_OUT_MONO,
                AudioFormat.ENCODING_PCM_16BIT,
                maxOf(minBuf, FRAME_BYTES * 4),
                AudioTrack.MODE_STREAM
            )
            track.play()
            try {
                while (nativeHandle != 0L) {
                    val pcm = GravitalTalkJni.nativeRecvAudio(nativeHandle) ?: break
                    track.write(pcm, 0, pcm.size)
                }
            } finally {
                track.stop()
                track.release()
            }
        }
    }

    // ─── Polling métricas ─────────────────────────────────────────────────────

    private fun startMetricsPolling() {
        metricsJob = viewModelScope.launch(Dispatchers.IO) {
            while (nativeHandle != 0L) {
                val m = GravitalTalkJni.nativeGetMetrics(nativeHandle)
                if (m.size >= 4) {
                    _metrics.value = PttMetrics(
                        rttMs = m[0],
                        jitterMs = m[1],
                        lossPercent = m[2],
                        estimatedMos = m[3],
                    )
                }
                delay(500)
            }
        }
    }

    // ─── Monitor estado del peer ──────────────────────────────────────────────

    private fun startPeerMonitor() {
        peerMonitorJob = viewModelScope.launch(Dispatchers.IO) {
            while (nativeHandle != 0L) {
                val active = GravitalTalkJni.nativeIsPeerPttActive(nativeHandle) != 0
                _isPeerPttActive.value = active
                delay(100)
            }
        }
    }

    // ─── Helpers ──────────────────────────────────────────────────────────────

    private fun stopAllTasks() {
        stopCapture()
        playbackJob?.cancel()
        metricsJob?.cancel()
        peerMonitorJob?.cancel()
        playbackJob = null
        metricsJob = null
        peerMonitorJob = null
    }

    private fun destroyNative() {
        val h = nativeHandle
        if (h != 0L) {
            GravitalTalkJni.nativeClose(h)
            GravitalTalkJni.nativeDestroy(h)
            nativeHandle = 0L
        }
    }

    // ─── Lifecycle ────────────────────────────────────────────────────────────

    override fun onCleared() {
        super.onCleared()
        val cm = appContext.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        runCatching { cm.unregisterNetworkCallback(networkCallback) }
        stopAllTasks()
        destroyNative()
        releaseWakeLock()
    }
}

class PttViewModelFactory(private val context: Context) : ViewModelProvider.Factory {
    override fun <T : ViewModel> create(modelClass: Class<T>): T {
        @Suppress("UNCHECKED_CAST")
        return PttViewModel(context.applicationContext) as T
    }
}
