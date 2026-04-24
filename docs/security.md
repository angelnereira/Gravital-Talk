# Modelo de amenazas

## 1. Alcance

Este documento cubre el modelo de amenazas del **protocolo** Gravital Sound y su implementación de referencia. No cubre despliegues productivos concretos (eso es responsabilidad del operador del relay).

## 2. Activos

- **Contenido de audio** — típicamente no confidencial, pero puede serlo (llamadas privadas).
- **Metadatos de sesión** — participantes, duración, calidad.
- **Infraestructura de relay** — CPU, ancho de banda, sockets.
- **Identidad** — quién está en qué sesión (relevante si hay integración con Gravital ID).

## 3. Atacantes considerados

| Atacante          | Capacidades                                         |
|-------------------|-----------------------------------------------------|
| **Observador de red pasivo** | Lee todo el tráfico en el camino.         |
| **Atacante in-path activo** | Lee, descarta, reordena, modifica paquetes.|
| **Off-path attacker** | No lee pero puede spoofear paquetes a cualquier destino. |
| **Participante malicioso** | Legítimo en la sesión, pero envía contenido hostil. |
| **Cliente abusivo** | Intenta agotar recursos del relay (DoS).      |

## 4. Propiedades de seguridad

### 4.1 v0.1 (estado actual)

| Propiedad                           | Estado | Nota                                              |
|-------------------------------------|--------|---------------------------------------------------|
| **Integridad por paquete**          | ✅     | CRC-16 detecta errores de transmisión, no ataques. |
| **Autenticación de origen**         | ❌     | Sin cripto. Cualquiera puede spoofear con `session_id` + `sequence` válidos. |
| **Confidencialidad**                | ❌     | Payload en texto claro.                            |
| **Resistencia a replay**            | Parcial | `sequence` + ventana de replay previene replay trivial dentro de una sesión, no inter-sesión. |
| **Resistencia a amplificación UDP** | ✅     | Handshake 3-way con cookie; el servidor nunca envía más bytes al cliente que los recibidos hasta confirmar ownership. |
| **Forward secrecy**                 | ❌     | No aplica hasta capa cripto.                       |
| **Aislamiento de sesiones**         | ✅     | `session_id` es un espacio de 2³². Colisiones prevenidas por el servidor. |

### 4.2 Roadmap (v0.4+)

- **Capa cripto opcional** vía handshake Noise (NK o XX pattern).
- **AEAD** (ChaCha20-Poly1305 o AES-GCM según target) para cifrar payload + autenticar header.
- **Rotación de claves** periódica dentro de la sesión.
- **Forward secrecy** con efímeras X25519.

## 5. Mitigaciones implementadas

### 5.1 Amplificación UDP

Un clásico ataque consiste en mandar un `HANDSHAKE_INIT` con IP spoofeada; si el servidor responde con un `HANDSHAKE_ACCEPT` mucho mayor, el atacante amplifica tráfico hacia la víctima. Mitigación:

- El `HANDSHAKE_ACCEPT` es estrictamente ≤ 1.1× el tamaño del `HANDSHAKE_INIT`.
- El servidor no asigna recursos (no entra a tabla de sesiones) hasta recibir `HANDSHAKE_CONFIRM` con `session_id` correcto.
- Rate limiting por IP de `HANDSHAKE_INIT` (100 req/s por default, configurable).

### 5.2 Exhaustación de memoria en fragmentación

Un atacante podría enviar fragmentos indefinidamente para inflar el reassembly buffer. Mitigación:

- Máximo 16 fragmentos por frame.
- Timeout de 500 ms para completar un frame; fragmentos huérfanos se liberan.
- Presupuesto total de memoria de reassembly por sesión: 32 KB.

### 5.3 Sequence number wraparound

El `sequence` es `u32` y envuelve tras ~2³² paquetes. A 50 paquetes/s (20 ms frames) eso son ~2.7 años; a 500 paquetes/s (2 ms) son ~100 días. La implementación detecta wraparound comparando diferencias módulo 2³² y aceptando paquetes con diferencia < 2³¹.

### 5.4 Decoder panics

El core tiene `#![deny(clippy::unwrap_used, clippy::panic)]` en release. Todos los paths de decode devuelven `Result`. Fuzzing continuo (post-0.1) con `cargo-fuzz` valida ausencia de panics.

### 5.5 Fingerprinting del protocolo

El magic `"GS"` + version hace al protocolo fácilmente identificable por DPI. En redes adversarias (censura estatal), esto puede ser un problema. Mitigación planificada:

- Obfuscation layer opcional (stream cipher sobre header + payload).
- Ejecución sobre WebSocket TLS para ocultar dentro de HTTPS.

## 6. Responsabilidades del operador

El operador del relay es responsable de:

- Restringir recursos por IP/token.
- Aplicar TLS a los listeners WebSocket.
- Rotar certificados.
- Logging con retención acotada (por GDPR/LOPD).
- Despliegue detrás de WAF si se expone a internet público.

## 7. Reporte de vulnerabilidades

Ver [`SECURITY.md`](../SECURITY.md).
