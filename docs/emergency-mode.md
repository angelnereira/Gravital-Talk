# Modo de emergencia y operación de 72 horas sin Internet

Guía técnica sobre cómo se comporta `Gravital-Talk` cuando caen Internet o la
red móvil, qué escenarios cubre hoy sin tocar una línea de servidor en la nube,
cómo dimensionar una celda local para **72 horas de autonomía** y qué límites
físicos y de protocolo conviene tener claros.

---

## 1. Principio de diseño: cero dependencia de la nube

La mayoría de las aplicaciones de mensajería y voz dependen de una cadena de
servicios remotos para empezar a funcionar: servidor de cuentas, directorio de
usuarios, API central, notificaciones push y pasarelas de medios. Si cae el
enlace con Internet, la aplicación queda inoperativa aunque dos teléfonos estén
a tres metros de distancia conectados al mismo router.

En `Gravital-Talk`:

- **El 100 % del protocolo vive en el dispositivo** (`gravital-talk-core` +
  `gravital-talk-transport` + `gravital-talk-codec`).
- **No existen cuentas ni servidores de inicio de sesión.** La identidad de una
  sala es un secreto simétrico efímero (`token`) generado localmente y
  entregado por código QR o enlace directo, y el acuerdo de claves se ejecuta
  entre los propios extremos (Noise `NNpsk0` / X25519 ECDH → HKDF-SHA256 →
  ChaCha20-Poly1305).
- **Los contactos se llaman sin servidor central.** Dos dispositivos que ya
  compartieron una sala conservan su secreto compartido y se avisan por la red
  local mediante datagramas UDP sellados (`knock`, puerto `47800`).
- **El descubrimiento público (STUN, UPnP y servidor de encuentro) es
  opcional.** Cuando no hay salida a Internet, falla de forma rápida y acotada
  y la sala opera íntegramente por la red local.

---

## 2. Escenarios operativos sin Internet (qué funciona hoy)

| Escenario | Qué hace falta | Alcance típico | Funciona hoy |
|---|---|---|---|
| **Punto de acceso desde un móvil** (sin datos móviles ni SIM) | Un teléfono activa «Zona Wi-Fi / Hotspot» y los demás se conectan a su Wi-Fi | 20–50 m | **Sí** |
| **Router Wi-Fi doméstico o de oficina sin WAN** (fibra cortada o ISP caído) | Alimentar el router con batería / UPS | 30–100 m (planta o edificio) | **Sí** |
| **Celda comunitaria o de brigada** (`gs-relay` en SBC + punto de acceso Wi-Fi en tejado) | Mini-PC / Raspberry Pi + AP exterior + batería / panel solar | 150–500 m (línea de vista) | **Sí** |
| **Enlace entre edificios sin Internet** (puente Wi-Fi punto a punto + `gs-relay` local) | Dos antenas direccionales en 5 GHz en puente transparente | 1–10 km (línea de vista) | **Sí** |
| **Salida de respaldo por satélite** (p. ej. Starlink en un centro coordinador hacia otra sede) | Un `gs-relay` accesible entre las sedes conectadas | Regional / global | **Sí** |

### Cómo opera una sala sin salida a Internet

1. **Descubrimiento de IP local sin ruta por defecto:**
   `discover_local_addr()` consulta la tabla de rutas del kernel primero hacia
   `8.8.8.8:53` y, si no existe puerta de enlace por defecto (`ENETUNREACH`,
   habitual en un móvil que hace de punto de acceso sin datos), sonda las
   subredes privadas locales (`192.168.0.0/16`, `10.42.0.0/24`,
   `172.20.10.0/28`, `10.0.0.0/8`, `172.16.0.0/12`, `169.254.0.0/16`) y
   enumera las interfaces de red activas.
2. **Apertura de sala con fallo rápido de STUN:**
   Si el DNS no resuelve o no hay ruta hacia el exterior, `discover_public_addr_on()`
   corta en el primer intento (≤ 1,2–2,8 s en el peor caso, < 5 ms sin ruta) y
   publica el QR con el endpoint LAN (`host=192.168.x.y&udp=9000`).
3. **Unión instantánea del invitado:**
   Al escanear una invitación que sólo contiene dirección local (sin `pub` ni
   `rv`), el invitado no consulta STUN y lanza el handshake directamente contra
   la IP local del anfitrión.
4. **Llamadas directas a contactos conocidos:**
   Si ambos dispositivos ya se habían emparejado antes, las llamadas viajan por
   difusión en la subred local (UDP `47800`, cifrado con ChaCha20-Poly1305) sin
   necesidad de volver a escanear ningún QR.

---

## 3. Dimensionamiento energético para 72 horas

Para mantener operativa una celda local de comunicación durante las **72 horas**
de autosuficiencia recomendadas por protección civil, la capacidad de batería
requerida se calcula mediante:

$$
E_{\text{Wh}} = \frac{P_{\text{W}} \times t_{\text{h}}}{\eta \times \text{DoD}}
$$

donde:
- $P_{\text{W}}$ es el consumo continuo en vatios,
- $t_{\text{h}} = 72\text{ h}$,
- $\eta \approx 0{,}85$ es la eficiencia global de conversión DC/DC (o inversor),
- $\text{DoD} \approx 0{,}90$ es la profundidad de descarga útil en baterías $\text{LiFePO}_4$.

| Configuración | Equipos alimentados | Consumo medio ($P_{\text{W}}$) | Energía útil ($72\text{ h}$) | Batería $\text{LiFePO}_4$ recomendada ($\eta=0{,}85$, $\text{DoD}=0{,}90$) |
|---|---|---|---|---|
| **Hogar / cuadrilla mínima** | Router Wi-Fi alimentado en DC (12 V) + recarga de 2 teléfonos | ~8 W | 576 Wh | **~750 Wh** (p. ej. estación portátil o batería 12 V 65 Ah) |
| **Celda comunitaria / edificio** | Punto de acceso Wi-Fi de alta cobertura + SBC con `gs-relay` + recarga de 5 teléfonos | ~15 W | 1.080 Wh | **~1.400 Wh** (o **~400 Wh** con un panel solar de 100 W que aporte 4 h pico/día) |
| **Centro de coordinación con enlace externo** | `gs-relay` + red Wi-Fi local + terminal satelital en uso programado | ~55 W | 3.960 Wh | **~5.200 Wh** (o **~1.800 Wh** con 400 Wp solares) |

> **Recomendación práctica:** alimentar el router y el nodo `gs-relay` directamente
> en corriente continua (**12 V / 5 V DC**) sin pasar por el inversor de
> 120 V / 230 V AC ahorra entre un 12 % y un 20 % de pérdidas en vacío durante
> las 72 horas.

### Levantar un `gs-relay` local en una red aislada

En cualquier equipo Linux de bajo consumo (Raspberry Pi, mini-PC o portátil)
conectado al punto de acceso:

```bash
# Enruta salas y actúa como punto de encuentro local en el puerto UDP 9000
gs-relay --bind 0.0.0.0 --udp-port 9000
```

Los participantes en la misma red local pueden unirse por QR directo (sin
necesidad de relay) o a una sala compartida en el relay:

```bash
gs ptt --relay 192.168.1.10:9000 --room brigada-1
```

---

## 4. Límites reales (qué NO hace hoy)

Es imprescindible distinguir lo que el protocolo resuelve hoy de lo que exige
capas adicionales de hardware o software:

1. **Necesita una red IP subyacente (Wi-Fi o Ethernet).**
   `Gravital-Talk` opera sobre sockets UDP/IP. Si dos teléfonos no comparten un
   punto de acceso Wi-Fi (o uno de ellos no abre su propio punto de acceso), no
   se descubren ni se envían paquetes por el aire por sí solos.
2. **El alcance es el del enlace IP.**
   Sin repetidores ni enlaces direccionales, el alcance es el de la Wi-Fi del
   router o del teléfono (20–100 m). No sustituye a una radio VHF/UHF o HF de
   varios kilómetros.
3. **No hay reenvío en malla (multi-salto) entre teléfonos.**
   Si el teléfono A ve al teléfono B y el teléfono B ve al C, pero A y C están
   fuera de alcance mutuo, B no reenvía automáticamente los paquetes de A hacia
   C (salvo que B esté ejecutando `gs-relay` y ambos enruten su sesión por él).
4. **No hay almacenamiento y reenvío (*store-and-forward*).**
   El audio es en tiempo real: si el destinatario está apagado o fuera de
   cobertura cuando alguien habla, esa intervención no se guarda para
   entregarla minutos u horas más tarde.
5. **El flujo de voz Opus actual no cabe en LoRa.**
   Una sesión de voz Opus a 16–24 kbps con paquetes cada 20 ms requiere del
   orden de 30–40 kbps brutos incluyendo cabeceras IP/UDP/Gravital. Los enlaces
   LoRa de largo alcance operan entre 0,3 y 5 kbps con restricciones de tiempo
   en el aire (*duty cycle*), por lo que no pueden transportar el canal de voz
   estándar.

---

## 5. Hoja de ruta técnica para resiliencia extendida

Las siguientes extensiones están diseñadas para encajar en la arquitectura
existente sin romper el protocolo de cable v1 (ver [`ADR-008`](./adr/008-offline-first-emergency-mode.md)):

1. **Transporte local sin router (Wi-Fi Direct / Wi-Fi Aware en Android):**
   levantar un grupo P2P local desde la propia app para que dos o más teléfonos
   creen su subred IP sin abrir manualmente los ajustes del sistema.
2. **Retransmisión ciega en malla (*Opportunistic Room Relay*):**
   permitir que cualquier nodo de una sala reenvíe datagramas cifrados con el
   mismo `session_id` (evitando bucles mediante la ventana de `sequence` del
   header de 24 B) sin poseer claves adicionales: el header ya viaja como AAD y
   el payload sigue cifrado de extremo a extremo.
3. **Notas PTT diferidas (*Store-and-Forward Voice Bursts*):**
   cuando ningún peer confirma recepción en tiempo real, empaquetar el turno de
   voz cifrado como un bloque firmado y reenviarlo automáticamente en cuanto
   reaparezca el contacto por `knock` o por la sala.
4. **Perfil de emergencia de ultra-bajo consumo:**
   espaciar los latidos de mantenimiento en redes locales estáticas (donde no
   hay NAT agresivo de operador que caduque en 20 s), activar DTX estricto y
   reducir el despertar de radio y pantalla al mínimo.
