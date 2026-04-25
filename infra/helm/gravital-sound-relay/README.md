# Helm chart — `gravital-sound-relay`

Despliega el relay productivo en Kubernetes.

## Install

```sh
helm install gs-relay ./infra/helm/gravital-sound-relay \
  --namespace gravital --create-namespace
```

Con scraping Prometheus vía ServiceMonitor (requiere prometheus-operator):

```sh
helm install gs-relay ./infra/helm/gravital-sound-relay \
  --namespace gravital --create-namespace \
  --set serviceMonitor.enabled=true
```

Con autoscaling:

```sh
helm install gs-relay ./infra/helm/gravital-sound-relay \
  --set autoscaling.enabled=true \
  --set autoscaling.maxReplicas=10
```

## Values destacados

- `service.externalTrafficPolicy: Local` — preserva la IP del cliente. Importante para rate limiting / abuse prevention.
- `securityContext` — `runAsNonRoot`, `readOnlyRootFilesystem`, drop ALL capabilities.
- `metricsService` separado para que Prometheus scrapee `/metrics` sin exponerlo al exterior.
- `serviceMonitor.enabled` — crea un ServiceMonitor compatible con prometheus-operator.

## Notas operacionales

- **NodePort vs LoadBalancer**: el chart por defecto usa LoadBalancer porque UDP necesita exposición directa. En clusters sin LB integrado (k3s, kind), cambiar a NodePort y exponer manualmente.
- **Múltiples replicas**: el routing es per-pod (en memoria). Para escalar horizontalmente con peers en distintos pods se necesita un backend compartido (Redis, etcd) — pendiente Track C futuro.
- **HPA por sesiones activas**: actualmente el HPA está basado en CPU. Para escalar por `gs_relay_active_sessions` se necesita prometheus-adapter + custom metric.
