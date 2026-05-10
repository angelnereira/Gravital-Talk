package com.gravitaltalk

import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioRecord
import android.media.AudioTrack
import android.media.MediaRecorder
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

data class PttMetrics(
    val rttMs: Float = 0f,
    val jitterMs: Float = 0f,
    val lossPercent: Float = 0f,
    val estimatedMos: Float = 4.5f,
)

sealed class PttConnectionState {
    object Idle : PttConnectionState()
    object Connecting : PttConnectionState()
    data class Connected(val sessionId: Int) : PttConnectionState()
    data class Error(val message: String) : PttConnectionState()
}

class PttViewModel : ViewModel() {

    companion object {
        private const val SAMPLE_RATE = 48_000
        private const val CHANNELS = 1
        private const val FRAME_DURATION_MS = 20
        private const val FRAME_SAMPLES = SAMPLE_RATE * FRAME_DURATION_MS / 1000  // 960
        private const val FRAME_BYTES = FRAME_SAMPLES * 2                           // 1920 (16-bit)
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

    // ─── Conexión ─────────────────────────────────────────────────────────────

    fun connect(relayHost: String, relayPort: Int = 9000) {
        if (_connectionState.value !is PttConnectionState.Idle &&
            _connectionState.value !is PttConnectionState.Error
        ) return

        viewModelScope.launch(Dispatchers.IO) {
            _connectionState.value = PttConnectionState.Connecting

            val handle = GravitalTalkJni.nativeCreate(SAMPLE_RATE, CHANNELS, 0)
            if (handle == 0L) {
                _connectionState.value = PttConnectionState.Error("Failed to create session")
                return@launch
            }
            nativeHandle = handle

            val status = GravitalTalkJni.nativeConnect(handle, relayHost, relayPort)
            if (status != 0) {
                GravitalTalkJni.nativeDestroy(handle)
                nativeHandle = 0L
                _connectionState.value = PttConnectionState.Error("Handshake failed: $status")
                return@launch
            }

            val sessionId = GravitalTalkJni.nativeGetSessionId(handle)
            _connectionState.value = PttConnectionState.Connected(sessionId)

            startPlayback()
            startMetricsPolling()
            startPeerMonitor()
        }
    }

    fun disconnect() {
        viewModelScope.launch(Dispatchers.IO) {
            stopCapture()
            captureJob?.cancel()
            playbackJob?.cancel()
            metricsJob?.cancel()
            peerMonitorJob?.cancel()

            val h = nativeHandle
            if (h != 0L) {
                GravitalTalkJni.nativeClose(h)
                GravitalTalkJni.nativeDestroy(h)
                nativeHandle = 0L
            }
            _isPttActive.value = false
            _isPeerPttActive.value = false
            _connectionState.value = PttConnectionState.Idle
        }
    }

    // ─── PTT ──────────────────────────────────────────────────────────────────

    fun pttPress() {
        val h = nativeHandle
        if (h == 0L || _connectionState.value !is PttConnectionState.Connected) return
        viewModelScope.launch(Dispatchers.IO) {
            GravitalTalkJni.nativePttPress(h)
            _isPttActive.value = true
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
                kotlinx.coroutines.delay(500)
            }
        }
    }

    // ─── Monitor estado del peer ──────────────────────────────────────────────

    private fun startPeerMonitor() {
        peerMonitorJob = viewModelScope.launch(Dispatchers.IO) {
            while (nativeHandle != 0L) {
                val active = GravitalTalkJni.nativeIsPeerPttActive(nativeHandle) != 0
                _isPeerPttActive.value = active
                kotlinx.coroutines.delay(100)
            }
        }
    }

    // ─── Lifecycle ────────────────────────────────────────────────────────────

    override fun onCleared() {
        super.onCleared()
        val h = nativeHandle
        if (h != 0L) {
            GravitalTalkJni.nativeClose(h)
            GravitalTalkJni.nativeDestroy(h)
            nativeHandle = 0L
        }
    }
}
