# Integración con Band-All

Gravital Talk usa [Band-All](https://github.com/angelnereira/Band-All) (servicio
TOTP/MFA en Rust) como **factor adicional de emparejamiento** en el modo sala.

Este documento existe por dos motivos:

1. documentar cómo se comporta Gravital Talk cuando usa Band-All, para que
   quien mantenga cualquiera de los dos proyectos sepa qué esperar;
2. **recopilar el comportamiento observado**, porque esos datos se usarán para
   mejorar Band-All.

La segunda parte no es una declaración de intenciones: las métricas que se citan
aquí salen de `BandAllClient` (código), se exponen por `/metrics` del relay y se
leen con Prometheus. Si un número de este documento no se puede reproducir
contra un relay en marcha, es que está desactualizado.

---

## Por qué esta integración existe

El modelo de autenticación de sala de Gravital Talk era un **token estático
compartido**: `gs_session_set_room_token(handle, token)` fija un PSK que todos
los participantes usan para el handshake Noise (`NNpsk0`). Tiene dos carencias:

| Carencia | Consecuencia |
|---|---|
| **El token no caduca** | Un token filtrado (captura, log, chat) sirve para siempre. |
| **El token *es* la credencial** | No hay segundo factor que añadir. |

Un código TOTP de 30 s arregla las dos: caduca solo, y sólo lo tiene quien está
mirando la pantalla en ese momento.

**Lo que no cambia:** sigue siendo un secreto compartido por todos los
participantes de la sala. TOTP autentica *el momento de entrada*, no a cada
persona por separado. Para identidad por participante har falta falta que
Band-All emita factores por sujeto, no por sala.

---

## Cómo funciona

```text
 participante                    relay                   Band-All
      │                            │                         │
      │  ClientHello               │                         │
      │  + código TOTP de sala     │                         │
      │───────────────────────────►│                         │
      │                            │  POST /v1/mfa/verify    │
      │                            │  {tenant, subject, code}│
      │                            │────────────────────────►│
      │                            │  200 {ok: true}         │
      │                            │◀────────────────────────│
      │  ServerHello + nota        │                         │
      │◀───────────────────────────│  handshake continúa     │
```

El código viaja en la negociación del handshake. **El audio no se ve afectado**:
la verificación ocurre una sola vez, al entrar. Una vez completado el handshake,
los paquetes de audio no vuelven a pasar por Band-All.

---

## Modos de fallo

La decisión de diseño más importante, y la razón por la que existe
`FailMode`:

**Si Band-All se cae, una sala en curso no pierde el audio.**

El default es `FailMode::Open` (admitir y registrar). Un fallo del segundo
factor tumbando una conversación en marcha es peor que admitir temporalmente
participantes: lo primero rompe la función del producto, lo segundo sólo
ensancha una ventana que la sesión ya tenía abierta.

| Modo | Si Band-All no responde | Cuándo usarlo |
|---|---|---|
| `Open` (default) | Admite el paquete, incrementa `admitted_open` | Producción normal |
| `OpenWithCounter` | Igual, con el contador visible para alertar | Cuando quieres un umbral de alerta |
| `Closed` | Rechaza el paquete, incrementa `rejected_closed` | Salas de alta sensibilidad |

Un código que Band-All **rechaza** (`Invalid`) se rechaza siempre, en los tres
modos. No es un fallo: es una respuesta.

---

## Comportamiento observado

Los números salen de `BandAllMetrics`, expuestos por `/metrics` del relay
(`gs_relay_bandall_valid`, `gs_relay_bandall_invalid`,
`gs_relay_bandall_unavailable`, `gs_relay_bandall_admitted_open`,
`gs_relay_bandall_latency_ms_mean`).

### Lo que se espera ver, y por qué

| Métrica | Valor esperado | Motivo |
|---|---|---|
| `bandall_latency_ms_mean` | < 200 ms | Es una llamada dentro de la misma red (compose/k8s). Si sube, el timeout de 1.5 s empieza a comerse el plazo del handshake. |
| `bandall_valid` / `bandall_invalid` | alto / bajo | Un código TOTP bien escrito casi siempre es válido. Una tasa de `invalid` alta significa que los usuarios no dan el código a tiempo. |
| `bandall_unavailable` | 0 | Cualquier valor aquí es un problema de red o de Band-All, no de los usuarios. |
| `bandall_admitted_open` | 0 | Sólo sube si Band-All falla. Es la métrica a alertar. |

### Lo que aún no se ha medido

Honesto, porque un documento que finge datos es peor que uno vacío:

- **Sesiones largas contra códigos cortos.** El patrón de Gravital Talk es
  sesiones de minutos u horas abiertas con un código de 30 s. No está medido
  cómo degrada eso la UX: si el usuario tiene que re-introducir el código, o si
  la sesión sobrevive sin más. **Este es el dato más valioso para Band-All**,
  porque ningún flujo de login humano normal ejercita el caso.
- **Deriva de reloj.** La ventana aceptada es de ±1 paso (`DRIFT_WINDOW_STEPS`).
  No hay datos de cuántos códigos se rechazan por deriva frente a por
  expiración real.
- **Concurrencia.** Varios participantes entrando a la vez con el mismo código.
  Band-All hace el claim atómico del paso (`cas_last_step`), así que se espera
  que exacto uno gane; no está verificado contra esta integración.

---

## Lo que se ha encontrado en el camino

Para que quede registro de lo aprendido, que es el material que sirve para
mejorar Band-All:

- **`/v1/mfa/verify` no distingue "código caducado" de "código incorrecto".**
  Los dos devuelven 401 uniforme (comportamiento correcto por seguridad, y
  documentado así en Band-All). El problema es de UX en Gravital Talk: el
  usuario no sabe si se equivocó al teclear o si fue demasiado lento. La
  mitigación del lado de Gravital Talk es mostrar el tiempo restante del código
  *antes* de pedirlo.

- **`enroll/confirm` consume el paso que acepta.** Confirmar y luego verificar
  con el mismo código es un replay y se deniega. Está documentado en Band-All,
  pero un cliente que no lo sepa se queda atascado. En Gravital Talk no se usa
  el enrolado (la sala ya existe), así que no aplica, pero se anota aquí porque
  es una trampa para el siguiente que integre.

---

## Cómo activarla

```toml
# relay.example.toml
[band_all]
base_url = "http://bandall:8080"
service_key = "…"          # credencial S2S
tenant_id = "gravital"
subject_id = "room-shared"
timeout_ms = 1500
fail_mode = "open"         # open | open_with_counter | closed
```

Sin esta sección, el relay funciona **exactamente igual que antes**: la
integración es opcional y el cliente desactivado no es un error
(`BandAllClient::verify_code` devuelve `Unavailable` sin configurar).

Band-All necesita, de su lado, un factor TOTP cuyo secreto sea el token de la
sala. Eso es trabajo de despliegue, no de código: la integración consume la API
existente, no añade endpoints a Band-All.

---

## Limitaciones conocidas

1. **No hay TLS en la llamada.** El cliente habla HTTP plano. Entre servicios
   dentro de la misma red es aceptable; si Band-All vive en otra red, hace falta
   un sidecar o reconsiderar el cliente.
2. **No reintenta.** Un 5xx de Band-All se traduce en `Unavailable` y se aplica
   el modo de fallo. No hay backoff: el handshake no puede esperar.
3. **Un factor por sala, no por participante.** Los códigos son
   intercambiables entre quienes conocen el token. Ver la nota de arriba.
