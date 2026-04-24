# ADR-002 · UDP como transporte primario

**Estado:** Aceptado (2026-04)

## Contexto

El transporte condiciona la latencia, el overhead y la portabilidad. Las opciones principales son TCP, UDP y QUIC. El medio debe soportar audio en tiempo real: mejor perder un paquete que esperar retransmisión.

## Decisión

- **UDP** es el transporte primario en todas las plataformas nativas.
- **WebSocket** (sobre TCP) es el transporte obligatorio para navegadores, por falta de UDP en el DOM.
- **QUIC/WebTransport** es el objetivo de migración post-0.1, cuando el soporte en navegadores se estabilice.

El trait `Transport` abstrae el medio, permitiendo combinar peers con transportes distintos (uno UDP, otro WebSocket) vía relay.

## Alternativas consideradas

### A. TCP en todas partes
- ✅ Fiable, fácil de debug, NAT-friendly.
- ❌ Retransmisiones de paquetes viejos añaden latencia catastrófica en audio real-time.
- ❌ Head-of-line blocking.
- **Rechazada.**

### B. QUIC directo en todos los peers
- ✅ Semántica datagram + control stream en un mismo canal. TLS integrado.
- ❌ Soporte móvil inmaduro (no en todas las versiones de iOS/Android).
- ❌ WebTransport aún no universal en navegadores.
- ❌ `quinn` + stack TLS añaden ~3-5 MB al binario.
- **Rechazada para 0.1.** Reevaluar en 0.3.

### C. SCTP
- ✅ Mensajes discretos + ordenamiento parcial configurable.
- ❌ Casi sin soporte en internet público (NATs bloquean).
- **Rechazada.**

## Consecuencias

- El código de transporte para navegadores es un caso especial (WebSocket) y no comparte path con el nativo.
- El jitter buffer debe auto-ajustarse cuando detecta transporte TCP (mayor latencia variable).
- NAT traversal es responsabilidad futura (STUN/TURN o relay explícito).

## Referencias

- [`transport.md`](../transport.md)
