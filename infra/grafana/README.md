# Grafana dashboards para Gravital Sound

Dashboards prediseñados que consumen métricas Prometheus expuestas por `gs-relay` en `/metrics`.

## Disponibles

- **`gravital-fleet-overview.json`** — visión global de todos los relays:
  - Stat panels: sesiones activas, conexiones WS, paquetes/s, throughput Mbit/s.
  - Timeseries: throughput in vs out, drop reasons, sesiones activas histórico.
  - Filtro por `instance` (Prometheus label).

## Instalación

### Opción 1: Helm (con stack `kube-prometheus-stack`)

Si usas el `kube-prometheus-stack`, monta los JSONs como ConfigMaps con la label `grafana_dashboard: "1"`:

```sh
kubectl create configmap gs-fleet-overview \
  -n monitoring \
  --from-file=infra/grafana/dashboards/gravital-fleet-overview.json
kubectl label configmap gs-fleet-overview \
  -n monitoring \
  grafana_dashboard=1
```

### Opción 2: Import manual desde la UI de Grafana

`Dashboards → Import → Upload JSON file → seleccionar el archivo`.

### Opción 3: Provisioning estático

```yaml
# /etc/grafana/provisioning/dashboards/gravital.yaml
apiVersion: 1
providers:
  - name: gravital
    folder: Gravital
    type: file
    options:
      path: /var/lib/grafana/dashboards/gravital
```

Y copiar los JSONs a `/var/lib/grafana/dashboards/gravital/`.

## Próximos dashboards (pendientes)

- **Per-Session Quality**: MOS, RTT, jitter, loss por session_id (requiere métricas extra del CodecSession).
- **Codec Performance**: encode/decode latency p50/p99/p99.9 (necesita histograma Prometheus, no incluido en MVP del relay).
- **Alertas**: relay down, > 5% loss sostenido, MOS < 3.5 — pendientes de definir reglas en Prometheus.
