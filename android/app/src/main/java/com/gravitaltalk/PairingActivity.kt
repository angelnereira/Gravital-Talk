package com.gravitaltalk

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import android.widget.Toast
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.core.content.ContextCompat
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.lifecycleScope
import com.google.zxing.BinaryBitmap
import com.google.zxing.MultiFormatReader
import com.google.zxing.NotFoundException
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.gravitaltalk.databinding.ActivityPairingBinding
import kotlinx.coroutines.launch
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

/**
 * Pantalla de emparejamiento P2P.
 *
 * Flujos:
 * 1. HOME     → botones "Crear llamada" / "Unirse a llamada"
 * 2. HOSTING  → muestra QR + código de texto, espera cliente
 * 3. JOINING  → cámara para escanear QR O campo de texto para relay manual
 * 4. CONNECTED→ lanza MainActivity con el handle nativo ya conectado
 */
class PairingActivity : AppCompatActivity() {

    private lateinit var binding: ActivityPairingBinding
    private lateinit var viewModel: PairingViewModel

    private var cameraExecutor: ExecutorService? = null
    private var cameraProvider: ProcessCameraProvider? = null
    private var cameraStarted = false
    private var qrHandled = false

    private val requestCamera = registerForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        if (granted) startCamera() else showJoinManual()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        supportActionBar?.hide()
        binding = ActivityPairingBinding.inflate(layoutInflater)
        setContentView(binding.root)

        viewModel = ViewModelProvider(this, PairingViewModelFactory(applicationContext))[PairingViewModel::class.java]

        setupButtons()
        setupTabs()
        observeState()

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

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        when (binding.viewFlipper.displayedChild) {
            SCREEN_HOST, SCREEN_JOIN -> viewModel.hangUp()
            else -> @Suppress("DEPRECATION") super.onBackPressed()
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        cameraExecutor?.shutdown()
        cameraExecutor = null
    }

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
                Toast.makeText(this, getString(R.string.hint_relay_host_port), Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            val lastColon = relay.lastIndexOf(':')
            if (lastColon < 0) {
                Toast.makeText(this, "Formato: host:puerto", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            val host = relay.substring(0, lastColon)
            val port = relay.substring(lastColon + 1).toIntOrNull() ?: run {
                Toast.makeText(this, "Puerto inválido", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            viewModel.joinFromRelay(host, port)
        }

        binding.btnBackToHome.setOnClickListener {
            viewModel.hangUp()
        }

        binding.btnJoinRoom.setOnClickListener {
            val addr = binding.etRoomRelay.text?.toString()?.trim()
            if (addr.isNullOrBlank()) {
                Toast.makeText(this, "Ingresa la dirección del relay", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            viewModel.joinFromRoom(addr)
        }
    }

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

    private fun requestCameraIfNeeded() {
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA)
            == PackageManager.PERMISSION_GRANTED
        ) {
            startCamera()
        } else {
            requestCamera.launch(Manifest.permission.CAMERA)
        }
    }

    private fun startCamera() {
        if (cameraStarted) return
        cameraStarted = true

        val executor = Executors.newSingleThreadExecutor()
        cameraExecutor = executor

        ProcessCameraProvider.getInstance(this).also { future ->
            future.addListener({
                val provider = future.get()
                cameraProvider = provider

                val preview = Preview.Builder().build().also {
                    it.setSurfaceProvider(binding.cameraPreview.surfaceProvider)
                }

                val analysis = ImageAnalysis.Builder()
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .build()

                analysis.setAnalyzer(executor) { imageProxy ->
                    if (qrHandled) { imageProxy.close(); return@setAnalyzer }
                    try {
                        val buffer = imageProxy.planes[0].buffer
                        val bytes = ByteArray(buffer.remaining())
                        buffer.get(bytes)
                        val w = imageProxy.width
                        val h = imageProxy.height

                        // Try original orientation first, then rotate 90° (portrait cameras)
                        val raw = decodeQr(bytes, w, h)
                            ?: decodeQr(rotateYuv90(bytes, w, h), h, w)

                        if (raw != null && !qrHandled) {
                            qrHandled = true
                            runOnUiThread { viewModel.joinFromQr(raw) }
                        }
                    } catch (_: Exception) {
                        // Ignore per-frame errors; try next frame
                    } finally {
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
                }.onFailure {
                    cameraStarted = false
                }
            }, ContextCompat.getMainExecutor(this))
        }
    }

    private fun decodeQr(bytes: ByteArray, w: Int, h: Int): String? {
        return try {
            val source = PlanarYUVLuminanceSource(bytes, w, h, 0, 0, w, h, false)
            val bmp = BinaryBitmap(HybridBinarizer(source))
            MultiFormatReader().decode(bmp).text
        } catch (_: NotFoundException) {
            null
        } catch (_: Exception) {
            null
        }
    }

    /** Rotate YUV Y-plane 90° clockwise for portrait-mode cameras. */
    private fun rotateYuv90(src: ByteArray, w: Int, h: Int): ByteArray {
        val dst = ByteArray(src.size)
        for (y in 0 until h) {
            for (x in 0 until w) {
                dst[x * h + (h - y - 1)] = src[y * w + x]
            }
        }
        return dst
    }

    private fun stopCamera() {
        cameraStarted = false
        cameraProvider?.unbindAll()
        cameraProvider = null
        cameraExecutor?.shutdown()
        cameraExecutor = null
    }

    private fun launchPttScreen(handle: Long) {
        val intent = Intent(this, MainActivity::class.java).apply {
            putExtra(EXTRA_NATIVE_HANDLE, handle)
            flags = Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP
        }
        startActivity(intent)
        finish()
    }

    companion object {
        const val EXTRA_NATIVE_HANDLE = "native_handle"

        private const val SCREEN_HOME = 0
        private const val SCREEN_HOST = 1
        private const val SCREEN_JOIN = 2
    }
}
