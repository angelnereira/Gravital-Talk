package com.gravitaltalk

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import android.widget.Toast
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.OptIn
import androidx.appcompat.app.AppCompatActivity
import androidx.camera.core.CameraSelector
import androidx.camera.core.ExperimentalGetImage
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.core.content.ContextCompat
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.lifecycleScope
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.common.InputImage
import com.gravitaltalk.databinding.ActivityPairingBinding
import kotlinx.coroutines.launch
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

/**
 * Pantalla de emparejamiento P2P.
 *
 * Flujos disponibles:
 * 1. HOME     → botones "Crear llamada" / "Unirse a llamada"
 * 2. HOSTING  → muestra QR + código de texto, espera cliente
 * 3. JOINING  → cámara para escanear QR O campo de texto para relay manual
 * 4. CONNECTED→ lanza MainActivity con el handle nativo ya conectado
 *
 * La actividad también acepta deep links `gravital-talk://pair?...`
 */
class PairingActivity : AppCompatActivity() {

    private lateinit var binding: ActivityPairingBinding
    private lateinit var viewModel: PairingViewModel

    private var cameraExecutor: ExecutorService? = null
    private var cameraStarted = false
    private var qrHandled = false

    // ── Permiso de cámara ──────────────────────────────────────────────────────

    private val requestCamera = registerForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        if (granted) startCamera() else showJoinManual()
    }

    // ── Lifecycle ──────────────────────────────────────────────────────────────

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityPairingBinding.inflate(layoutInflater)
        setContentView(binding.root)

        viewModel = ViewModelProvider(this, PairingViewModelFactory(applicationContext))[PairingViewModel::class.java]

        setupButtons()
        setupTabs()
        observeState()

        // Manejar deep link (gravital-talk://pair?...) si llegamos por ese camino.
        intent?.data?.let { uri ->
            if (uri.scheme == "gravital-talk" && uri.host == "pair") {
                viewModel.joinFromQr(uri.toString())
            }
        }
    }

    override fun onNewIntent(intent: Intent?) {
        super.onNewIntent(intent)
        intent?.data?.let { uri ->
            if (uri.scheme == "gravital-talk" && uri.host == "pair") {
                qrHandled = false
                viewModel.joinFromQr(uri.toString())
            }
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        cameraExecutor?.shutdown()
    }

    // ── Tabs ───────────────────────────────────────────────────────────────────

    private fun setupTabs() {
        binding.tabHost.addOnTabSelectedListener(object : com.google.android.material.tabs.TabLayout.OnTabSelectedListener {
            override fun onTabSelected(tab: com.google.android.material.tabs.TabLayout.Tab?) {
                if (tab?.position == 1) {
                    binding.cameraPreview.visibility = android.view.View.GONE
                    binding.layoutManual.visibility = android.view.View.VISIBLE
                    stopCamera()
                } else {
                    binding.cameraPreview.visibility = android.view.View.VISIBLE
                    binding.layoutManual.visibility = android.view.View.GONE
                    requestCameraIfNeeded()
                }
            }
            override fun onTabUnselected(tab: com.google.android.material.tabs.TabLayout.Tab?) {}
            override fun onTabReselected(tab: com.google.android.material.tabs.TabLayout.Tab?) {}
        })
    }

    // ── Botones ────────────────────────────────────────────────────────────────

    private fun setupButtons() {
        binding.btnCreateCall.setOnClickListener {
            val relay = binding.etRelayOptional.text?.toString()?.trim()
            viewModel.startHosting(relayUrl = relay?.ifBlank { null })
        }

        binding.btnJoinCall.setOnClickListener {
            showJoinScreen()
        }

        binding.btnCancelHost.setOnClickListener {
            viewModel.hangUp()
        }

        binding.btnJoinManual.setOnClickListener {
            val relay = binding.etManualRelay.text?.toString()?.trim()
            if (relay.isNullOrBlank()) {
                Toast.makeText(this, getString(R.string.hint_relay_optional), Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            val (host, port) = parseHostPort(relay) ?: run {
                Toast.makeText(this, "Formato: host:puerto", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            viewModel.joinFromRelay(host, port)
        }

        binding.btnBackToHome.setOnClickListener {
            viewModel.hangUp()
        }
    }

    // ── Observar estado ────────────────────────────────────────────────────────

    private fun observeState() {
        lifecycleScope.launch {
            viewModel.uiState.collect { state ->
                when (state.screen) {
                    is PairingScreen.Home -> showHome()
                    is PairingScreen.Hosting -> showHostScreen(state)
                    is PairingScreen.Joining -> showJoinScreen()
                    is PairingScreen.Connected -> launchPttScreen(state.screen.handle)
                }

                state.error?.let { err ->
                    Toast.makeText(this@PairingActivity, err, Toast.LENGTH_LONG).show()
                }
            }
        }
    }

    // ── Transiciones de pantalla ───────────────────────────────────────────────

    private fun showHome() {
        binding.viewFlipper.displayedChild = SCREEN_HOME
        stopCamera()
    }

    private fun showHostScreen(state: PairingUiState) {
        binding.viewFlipper.displayedChild = SCREEN_HOST
        binding.tvStatusHost.text = state.statusMsg
        binding.tvTextCode.text = state.textCode
        state.qrBitmap?.let { binding.ivQrCode.setImageBitmap(it) }
    }

    private fun showJoinScreen() {
        binding.viewFlipper.displayedChild = SCREEN_JOIN
        qrHandled = false
        requestCameraIfNeeded()
    }

    private fun showJoinManual() {
        binding.tabHost.getTabAt(1)?.select()
    }

    // ── QR scan (CameraX) ──────────────────────────────────────────────────────

    private fun requestCameraIfNeeded() {
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA)
            == PackageManager.PERMISSION_GRANTED
        ) {
            startCamera()
        } else {
            requestCamera.launch(Manifest.permission.CAMERA)
        }
    }

    @OptIn(ExperimentalGetImage::class)
    private fun startCamera() {
        if (cameraStarted) return
        cameraExecutor = Executors.newSingleThreadExecutor()

        val cameraProviderFuture = ProcessCameraProvider.getInstance(this)
        cameraProviderFuture.addListener({
            val provider = cameraProviderFuture.get()

            val preview = Preview.Builder().build().also {
                it.setSurfaceProvider(binding.cameraPreview.surfaceProvider)
            }

            val analysis = ImageAnalysis.Builder()
                .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                .build()

            val scanner = BarcodeScanning.getClient()

            analysis.setAnalyzer(cameraExecutor!!) { imageProxy ->
                if (qrHandled) {
                    imageProxy.close()
                    return@setAnalyzer
                }
                val mediaImage = imageProxy.image
                if (mediaImage != null) {
                    val img = InputImage.fromMediaImage(mediaImage, imageProxy.imageInfo.rotationDegrees)
                    scanner.process(img)
                        .addOnSuccessListener { barcodes ->
                            barcodes.firstOrNull { it.format == Barcode.FORMAT_QR_CODE }
                                ?.rawValue
                                ?.let { raw ->
                                    if (!qrHandled) {
                                        qrHandled = true
                                        runOnUiThread { viewModel.joinFromQr(raw) }
                                    }
                                }
                        }
                        .addOnCompleteListener { imageProxy.close() }
                } else {
                    imageProxy.close()
                }
            }

            runCatching {
                provider.unbindAll()
                provider.bindToLifecycle(
                    this,
                    CameraSelector.DEFAULT_BACK_CAMERA,
                    preview,
                    analysis,
                )
                cameraStarted = true
            }
        }, ContextCompat.getMainExecutor(this))
    }

    private fun stopCamera() {
        cameraStarted = false
        runCatching { ProcessCameraProvider.getInstance(this).get().unbindAll() }
    }

    // ── Lanzar PTT screen ──────────────────────────────────────────────────────

    private fun launchPttScreen(handle: Long) {
        val intent = Intent(this, MainActivity::class.java).apply {
            putExtra(EXTRA_NATIVE_HANDLE, handle)
            flags = Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP
        }
        startActivity(intent)
        finish()
    }

    // ── Utils ──────────────────────────────────────────────────────────────────

    private fun parseHostPort(addr: String): Pair<String, Int>? {
        val lastColon = addr.lastIndexOf(':')
        if (lastColon < 0) return null
        val host = addr.substring(0, lastColon)
        val port = addr.substring(lastColon + 1).toIntOrNull() ?: return null
        return Pair(host, port)
    }

    companion object {
        const val EXTRA_NATIVE_HANDLE = "native_handle"

        private const val SCREEN_HOME = 0
        private const val SCREEN_HOST = 1
        private const val SCREEN_JOIN = 2
    }
}
