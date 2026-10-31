# Relay en Railway (Gravital Cloud)

El relay de Gravital Talk (`gs-relay`) corre en Railway en el proyecto
**gravital-talk**, servicio **gs-relay**, y se publica en
`https://talk.gravitalcloud.com`.

## Por qué WebSocket

Railway publica un puerto HTTP(S) (y proxies TCP), pero **no deja entrar UDP**.
El protocolo va por UDP, así que en Railway el audio entra por WebSocket:

```
app ──UDP──▶ 127.0.0.1:N (puente gs_ws_bridge) ──wss──▶ talk.gravitalcloud.com/ws ──▶ relay
```

- El relay trata cada WebSocket como un peer más y lo encamina con los peers
  UDP de la misma sala (`ws::serve`).
- Los paquetes viajan sellados igual que por UDP: el TLS es una capa más y el
  relay no puede leer el audio.
- Lo que **no** funciona en Railway: el servidor de encuentro para perforar NAT
  (`rv=`, UDP) y gRPC. La app no los usa con un relay web.
- TCP añade algo de retardo frente a UDP y una pérdida de paquete se convierte
  en espera (no en hueco). Es el precio de no tener UDP; un VPS con UDP
  (`infra/terraform`) sigue siendo la opción de menor latencia.

## Rutas

| Ruta | Qué |
|---|---|
| `GET /healthz` | salud (lo usa Railway) |
| `GET /metrics` | Prometheus |
| `POST/GET/DELETE /api/rooms[/{code}]` | API de salas |
| `GET /ws` (upgrade) | audio por WebSocket |

## Despliegue

`railway.toml` (raíz) apunta a `docker/Dockerfile.railway`, que compila sólo
`gs-relay`. Railway reconstruye al hacer push en la rama conectada.

Variables del servicio:

| Variable | Valor | Para qué |
|---|---|---|
| `PORT` | `9100` | puerto HTTP que publica Railway |
| `GS_TRUST_PROXY` | `true` | IP real del cliente desde `X-Forwarded-For` |
| `GS_RATE_LIMIT` | `400` | paquetes/s por IP (0 = sin límite) |
| `GS_LOG` | `info` | nivel de log |

## Dominio

`talk.gravitalcloud.com` es un dominio propio del servicio. Necesita un
registro DNS `CNAME talk → <destino que indica Railway>` en el proveedor de
`gravitalcloud.com` (si es Cloudflare, en modo *DNS only*). Mientras no exista,
el servicio responde en su dominio `*.up.railway.app`.

## Comprobar

```bash
curl https://talk.gravitalcloud.com/healthz
curl -XPOST -d '{"session_id":42}' https://talk.gravitalcloud.com/api/rooms
cargo test -p gravital-talk-ffi --test ws_relay   # puente + relay en local
```
