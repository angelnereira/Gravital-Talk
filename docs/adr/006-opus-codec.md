# ADR 006 — Opus como codec de audio principal

**Estado**: Aceptado  
**Fecha**: 2026-04-24  
**Autores**: Angel Nereira

## Contexto

El MVP de Gravital Sound usaba PCM i16 raw como único formato de audio. Esto es adecuado para redes de alta capacidad pero inviable para Internet pública: a 48 kHz mono 20 ms, un frame PCM ocupa 1920 bytes (768 kbps) frente al típico budget de VoIP de 64–128 kbps.

Se necesita un codec de voz que:
- Funcione a 8–48 kHz.
- Tenga latencia algorítmica baja (< 30 ms).
- Soporte FEC (Forward Error Correction) para tolerar pérdidas de hasta 20%.
- Tenga licencia libre para uso comercial y embebido.
- Tenga bindings Rust maduros.

## Decisión

Se adopta **Opus** (RFC 6716, libopus 1.x) como codec primario de voz. Implementado vía `audiopus = "0.2"` (wrapper seguro sobre la C API de libopus).

## Argumentos a favor

- **Licencia BSD** — libre para cualquier uso, incluido comercial y embebido.
- **Estándar IETF** — RFC 6716 + 7587, adoptado en WebRTC, Discord, Teams.
- **Latencia algorítmica** — 26.5 ms a 48 kHz (mejor que AAC-LD, similar a G.722.2).
- **FEC integrado** — recupera 1 paquete perdido sin retransmisión.
- **CBR y VBR** — adapta bitrate a condiciones de red.
- **Soporte `no_std`** — libopus compila para targets embebidos (con adaptaciones de heap).
- **SNR excelente** — > 20 dB a 64 kbps en voz conversacional.

## Alternativas descartadas

| Codec | Razón del descarte |
|-------|--------------------|
| AAC-LC | Latencia 80–100 ms, requiere licencia por encoder |
| G.711 (PCMU/PCMA) | Compresión mínima (64 kbps), sin FEC |
| Speex | Superado por Opus en todos los parámetros |
| EVS | Complejo de implementar, sin bindings Rust maduros |

## Consecuencias

- La compilación del workspace en Linux requiere `libopus-dev` y `pkg-config`.
- En macOS `opus` está disponible via `brew install opus`.
- Los targets `wasm32` y targets sin pkg-config deben compilar con `--no-default-features` (excluye feature `opus`).
- El codec se expone como feature gate: `gravital-sound/opus` (activado por defecto).
