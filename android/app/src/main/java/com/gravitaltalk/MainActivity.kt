package com.gravitaltalk

import android.Manifest
import android.content.pm.PackageManager
import android.os.Bundle
import android.view.MotionEvent
import android.view.View
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.lifecycleScope
import com.gravitaltalk.databinding.ActivityMainBinding
import com.google.android.material.snackbar.Snackbar
import kotlinx.coroutines.launch

class MainActivity : AppCompatActivity() {

    private lateinit var binding: ActivityMainBinding
    private lateinit var viewModel: PttViewModel

    private val requestPermission = registerForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        if (!granted) {
            Snackbar.make(binding.root, "Se requiere permiso de micrófono", Snackbar.LENGTH_LONG).show()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)

        viewModel = ViewModelProvider(this)[PttViewModel::class.java]

        ensureMicPermission()
        setupUi()
        observeState()
    }

    // ─── Permisos ─────────────────────────────────────────────────────────────

    private fun ensureMicPermission() {
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.RECORD_AUDIO)
            != PackageManager.PERMISSION_GRANTED
        ) {
            requestPermission.launch(Manifest.permission.RECORD_AUDIO)
        }
    }

    // ─── UI setup ─────────────────────────────────────────────────────────────

    private fun setupUi() {
        // Botón Conectar / Desconectar.
        binding.btnConnect.setOnClickListener {
            val state = viewModel.connectionState.value
            if (state is PttConnectionState.Idle || state is PttConnectionState.Error) {
                val relay = binding.etRelay.text?.toString()?.trim().orEmpty()
                if (relay.isEmpty()) {
                    Snackbar.make(binding.root, "Ingresa el host del relay", Snackbar.LENGTH_SHORT).show()
                    return@setOnClickListener
                }
                viewModel.connect(relay)
            } else {
                viewModel.disconnect()
            }
        }

        // Botón PTT — mantener presionado para hablar.
        binding.btnPtt.setOnTouchListener { _, event ->
            when (event.action) {
                MotionEvent.ACTION_DOWN -> {
                    viewModel.pttPress()
                    true
                }
                MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                    viewModel.pttRelease()
                    true
                }
                else -> false
            }
        }
    }

    // ─── Observar estado ──────────────────────────────────────────────────────

    private fun observeState() {
        lifecycleScope.launch {
            viewModel.connectionState.collect { state ->
                updateConnectionUi(state)
            }
        }

        lifecycleScope.launch {
            viewModel.isPttActive.collect { active ->
                updatePttButton(active)
            }
        }

        lifecycleScope.launch {
            viewModel.isPeerPttActive.collect { active ->
                binding.tvPeerStatus.text = if (active) "● Peer transmitiendo" else ""
            }
        }

        lifecycleScope.launch {
            viewModel.metrics.collect { m ->
                binding.tvMetrics.text =
                    "RTT %.0fms  Jitter %.0fms  Loss %.1f%%  MOS %.1f".format(
                        m.rttMs, m.jitterMs, m.lossPercent, m.estimatedMos
                    )
            }
        }
    }

    private fun updateConnectionUi(state: PttConnectionState) {
        when (state) {
            is PttConnectionState.Idle -> {
                binding.tvStatus.text = getString(R.string.status_idle)
                binding.tvSessionId.text = ""
                binding.btnConnect.text = getString(R.string.btn_connect)
                binding.btnPtt.isEnabled = false
                binding.tilRelay.isEnabled = true
                binding.tilRoom.isEnabled = true
            }
            is PttConnectionState.Connecting -> {
                binding.tvStatus.text = getString(R.string.status_connecting)
                binding.btnConnect.isEnabled = false
                binding.btnPtt.isEnabled = false
                binding.tilRelay.isEnabled = false
                binding.tilRoom.isEnabled = false
            }
            is PttConnectionState.Connected -> {
                binding.tvStatus.text = getString(R.string.status_connected)
                binding.tvSessionId.text = "session 0x%08X".format(state.sessionId)
                binding.btnConnect.text = getString(R.string.btn_disconnect)
                binding.btnConnect.isEnabled = true
                binding.btnPtt.isEnabled = true
                binding.tilRelay.isEnabled = false
                binding.tilRoom.isEnabled = false
            }
            is PttConnectionState.Error -> {
                binding.tvStatus.text = "Error: ${state.message}"
                binding.btnConnect.text = getString(R.string.btn_connect)
                binding.btnConnect.isEnabled = true
                binding.btnPtt.isEnabled = false
                binding.tilRelay.isEnabled = true
                binding.tilRoom.isEnabled = true
                Snackbar.make(binding.root, state.message, Snackbar.LENGTH_LONG).show()
            }
        }
    }

    private fun updatePttButton(active: Boolean) {
        if (active) {
            binding.btnPtt.text = getString(R.string.ptt_release)
            binding.btnPtt.backgroundTintList =
                ContextCompat.getColorStateList(this, R.color.ptt_transmit)
        } else {
            binding.btnPtt.text = getString(R.string.ptt_press)
            binding.btnPtt.backgroundTintList =
                ContextCompat.getColorStateList(this, R.color.ptt_idle)
        }
    }
}
