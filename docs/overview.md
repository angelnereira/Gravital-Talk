# Gravital Sound — Visión general

## Propósito

Gravital Sound es un **protocolo de transporte de audio en tiempo real sobre internet**, diseñado para los casos en los que WebRTC es demasiado pesado o poco transparente, RTP carece de un control de sesión moderno, y las soluciones propietarias (Zoom, Discord, Teams) no son portables a infraestructura propia.

Objetivos:

1. **Latencia baja y predecible.** p99 end-to-end por debajo de 25 ms en LAN, p50 por debajo de 5 ms en loopback. El jitter buffer es explícito y configurable, no escondido.
2. **Portabilidad universal.** El mismo protocolo corre en Linux (servidor y escritorio), macOS, Windows, Android, iOS, navegador (WASM) y sistemas embebidos (musl, `no_std`).
3. **Transparencia.** La especificación binaria es pública, simple y suficiente para que un tercero escriba una implementación compatible sin consultar el código fuente.
4. **Integridad.** CRC-16 obligatorio en cada paquete; extensión cripto opcional en una fase posterior.
5. **Observabilidad.** Métricas de RTT, jitter, pérdida, reordenamiento y MOS estimado expuestas sin acoplar a una stack de observabilidad concreta.

## No objetivos

- **No es un reemplazo de WebRTC** para escenarios navegador-a-navegador con NAT traversal automático. Gravital Sound asume conectividad directa o un relay explícito.
- **No incluye transporte confiable.** El medio por defecto es UDP *best effort*. Las aplicaciones que necesiten confiabilidad deben usar el canal de control o capa superior.
- **No es un codec.** El core transporta frames opacos; el codec (Opus en la siguiente fase, PCM en el MVP) es una capa independiente.
- **No es un framework de aplicación.** No hay UI, no hay lógica de sala, no hay gestión de participantes más allá del handshake punto a punto o relay.

## Casos de uso objetivo

| Caso | Descripción |
|------|-------------|
| **Broadcasting en tiempo real** | Estudios que envían audio en vivo a un CDN o a un ingestor propio. |
| **Telefonía empresarial** | Backends de PBX custom que necesitan control total del stack de transporte. |
| **Sincronización de músicos remotos** | Jam sessions latencia-crítica con buffering explícito. |
| **Voz en juegos multi-plataforma** | Mismo protocolo en servidor Linux, cliente iOS/Android y navegador (WASM). |
| **Ingesta IoT de audio** | Micrófonos embebidos Linux/musl que publican audio a un colector. |
| **Infraestructura Gravital** | Integración opcional con Gravital ID (auth) y Gravital Cloud (persistencia de métricas). |

## Posicionamiento

| Aspecto | RTP | WebRTC | Opus Native | **Gravital Sound** |
|---------|-----|--------|-------------|---------------------|
| Portabilidad server → browser | ❌ | ✅ | ❌ | ✅ |
| Especificación simple | ✅ | ❌ | ✅ | ✅ |
| Métricas nativas | Parcial (RTCP) | ✅ | ❌ | ✅ |
| Integridad por paquete | ❌ | ✅ (SRTP) | ❌ | ✅ |
| Codec-agnóstico | ✅ | ❌ | ❌ | ✅ |
| SDKs idiomáticos | Fragmentado | Fragmentado | ❌ | ✅ |
| Dependencia de servicios | — | Señalización STUN/TURN | — | Ninguna |

## Roadmap resumido

- **0.1** Protocolo core + transporte UDP/WebSocket + SDKs Python/Web.
- **0.2** Codec Opus, audio I/O (`cpal`), SDK Swift e SDK Kotlin.
- **0.3** Relay productivo con Docker, NAT traversal, multicast.
- **0.4** Capa cripto (handshake Noise, integridad AEAD).
- **1.0** Protocolo estable, SemVer compliance, auditoría de seguridad externa.

Detalle completo de fases en [`seed.md`](../seed.md) y [`CHANGELOG.md`](../CHANGELOG.md).
