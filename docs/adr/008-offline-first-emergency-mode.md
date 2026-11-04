# ADR-008 · Operación sin Internet y modo de emergencia (72 h)

**Estado:** Aceptado (2026-11)

## Contexto

Las directrices internacionales de preparación civil para escenarios de **72 horas sin servicios esenciales** (Estrategia de Preparación de la Unión Europea 2025–2026 y guías nacionales como la mochila de 72 horas de SINAPROC en Panamá) identifican un punto único de fallo recurrente en el software moderno: las aplicaciones de comunicación asumen la disponibilidad permanente de Internet, DNS, servidores de cuentas y plataformas en la nube, por lo que dejan de funcionar incluso cuando los dispositivos de los usuarios y una red local siguen encendidos.

`Gravital-Talk` nació con el protocolo 100 % en el dispositivo, sin cuentas ni nube obligatoria. Sin embargo, al auditar la apertura y unión de salas en redes aisladas sin salida a Internet (router alimentado por batería sin WAN, punto de acceso Wi-Fi de un teléfono sin datos móviles, o microred comunitaria), aparecieron tres obstáculos reales:

1. **Descubrimiento de IP local dependiente de la ruta por defecto:** `discover_local_addr()` conectaba un socket UDP a `8.8.8.8:53` para preguntar al kernel qué interfaz salía a Internet. En un teléfono que actúa como punto de acceso Wi-Fi sin datos móviles o en un router sin puerta de enlace por defecto (`0.0.0.0/0`), `connect(8.8.8.8)` falla con `ENETUNREACH`, devolviendo `0.0.0.0` e impidiendo abrir la sala.
2. **Espera innecesaria en STUN cuando no hay salida a Internet:** `discover_public_addr_on()` consultaba tres servidores STUN secuenciales con timeouts que sumaban hasta 9,6 s, superando el tope de 6 s de la app y reteniendo el socket de la sesión cuando arrancaba el handshake local.
3. **Ausencia de distinción entre invitaciones locales y remotas al unirse:** un invitado que escaneaba un QR puramente local (sin `pub` ni `rv`) ejecutaba igualmente STUN antes de conectar por la LAN.

## Decisión

1. **Descubrimiento de dirección LAN sin ruta por defecto:**
   - Si `connect(8.8.8.8:53)` no devuelve una interfaz válida, `discover_local_addr()` sonda en la tabla de rutas del kernel las subredes privadas y de punto de acceso móvil (`192.168.0.0/16`, `10.42.0.0/24`, `172.20.10.0/28`, `10.0.0.0/8`, `172.16.0.0/12`, `169.254.0.0/16`), sin emitir tráfico de red, y la app respalda el resultado con la enumeración de interfaces activas (`NetworkInterface.list`).
2. **Fallo rápido y acotado de STUN en el socket de la sesión:**
   - `discover_public_addr_on()` aplica un tope global estricto de 2,8 s, corta en el primer fallo o timeout de DNS sin repetir contra los demás servidores de la misma zona, y aborta en el acto si `send_to` devuelve red inalcanzable (`ENETUNREACH`).
3. **Unión inmediata en invitaciones de red local:**
   - Cuando la invitación escaneada o abierta carece de dirección pública (`pub`) y de servidor de encuentro (`rv`), el invitado omite STUN y conecta directamente a la IP local del anfitrión.
4. **Hoja de ruta de resiliencia de 72 horas (`docs/emergency-mode.md`):**
   - Documentar con precisión qué escenarios de corte de infraestructura cubre `Gravital-Talk` hoy, cómo dimensionar energéticamente una celda comunitaria (`gs-relay` + AP Wi-Fi) para 72 horas, cuáles son los límites físicos actuales y qué extensiones (Wi-Fi Direct, malla de paquetes cifrados en capa de sala y PTT diferido) completan la arquitectura *offline-first*.

## Alternativas consideradas

### A. Mantener `connect(8.8.8.8:53)` y exigir puerta de enlace configurada en el router
- ✅ Cero cambios en `stun.rs`.
- ❌ Un teléfono en modo *hotspot* sin datos móviles no instala ruta por defecto en el kernel y no puede alojar salas en una emergencia.
- **Rechazada.**

### B. Eliminar STUN por defecto al crear una sala y pedir un interruptor manual «Salir a Internet»
- ✅ Apertura instantánea sin red.
- ❌ Empeora el caso cotidiano entre redes distintas cuando sí hay Internet y el usuario olvida activar el interruptor.
- **Rechazada:** es preferible el fallo rápido automático cuando no hay salida.

### C. Integrar transporte LoRa directo para el flujo de voz Opus actual
- ✅ Largo alcance con muy bajo consumo energético.
- ❌ El flujo de voz Opus de la app (~16–24 kbps más cabeceras de 24 B + tag AEAD de 16 B a 50 paquetes/s) excede en un orden de magnitud la capacidad efectiva de LoRa en modos de largo alcance (0,3–5 kbps con límites de ciclo de trabajo).
- **Rechazada para el canal de voz estándar:** cualquier transporte sub-2 kbps requerirá un perfil de códec específico de muy baja tasa (p. ej. Codec2 o ráfagas diferidas comprimidas).

## Consecuencias

- Crear y unirse a una sala en una Wi-Fi sin salida a Internet o sobre el punto de acceso de un teléfono en modo avión + Wi-Fi funciona sin errores y sin retener el socket de la sesión.
- Queda documentada la arquitectura de despliegue autónomo para 72 horas y la separación estricta entre capacidades actuales y futuras en [`emergency-mode.md`](../emergency-mode.md).

## Referencias

- [`emergency-mode.md`](../emergency-mode.md)
- [`pairing.md`](../pairing.md)
- `crates/gravital-talk-transport/src/stun.rs`
