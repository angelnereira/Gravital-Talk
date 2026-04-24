# ADR-004 · Opus como codec por default (roadmap)

**Estado:** Aceptado en principio, **no implementado en 0.1**.

## Contexto

El protocolo es codec-agnóstico: transporta frames opacos. Sin embargo, necesita un default razonable para que los SDKs funcionen out-of-the-box. Evaluamos Opus, AAC-LC, G.722, Speex y PCM crudo.

## Decisión

- **Opus** será el codec por default en 0.2+.
- **PCM crudo** es el único codec soportado en 0.1 (simplifica la trayectoria al primer release).
- El número de codec en el handshake (`codec_preferred`) reserva `0x01 = Opus`, `0x02 = PCM`, dejando espacio para otros.

## Justificación

- **Opus** es el estándar de facto para audio real-time por internet (RFC 6716). Licencia permisiva, implementación libopus madura. Rango 6 kbps – 510 kbps, hasta 48 kHz, mono/stereo, NB/WB/FB.
- Integración: crate `opus` con feature `vendored` para compilar libopus desde source — evita depender de libopus del sistema.
- En WASM, libopus tiene port probado a través de `emscripten` + wasm-bindgen.

## Alternativas

| Codec | Descripción | Veredicto |
|-------|-------------|-----------|
| **AAC-LC** | Alto quality, ampliamente soportado en hardware móvil. | Patentes. Licencia restrictiva. Rechazado. |
| **G.722** | Telefonía clásica. | Obsoleto, menor calidad. Rechazado. |
| **Speex** | Predecesor de Opus. | Deprecated. Rechazado. |
| **Opus** | Estándar moderno real-time. | **Aceptado.** |

## Consecuencias

- En 0.1, los ejemplos usan PCM — los tamaños de paquete son grandes (~2 KB por frame de 20 ms stereo 48 kHz), aceptable en LAN pero ineficiente en WAN. Esto está documentado en el README.
- En 0.2, el crate `gravital-sound-codec` añade libopus vendored.
- La API FFI ya incluye el campo `codec` en `GsConfig` con los valores reservados.

## Referencias

- RFC 6716 (Opus)
- [`seed.md`](../../seed.md) §4.1
