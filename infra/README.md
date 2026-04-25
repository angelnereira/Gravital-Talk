# Gravital Sound — Infraestructura as Code

Módulos Terraform y scripts cloud-init para desplegar Gravital Sound en cualquier server o dispositivo en pocos minutos.

## Estructura

```
infra/
├── terraform/
│   ├── modules/
│   │   ├── relay-aws/             # EC2 + Security Group + (opt) Route53
│   │   ├── relay-hetzner/         # Hetzner Cloud (opción barata, ~€4/mes)
│   │   ├── relay-digitalocean/    # Droplet + DO Firewall
│   │   └── edge-node/             # user_data agnóstico para edge clients
│   └── examples/
│       ├── single-region-aws/     # Relay en AWS us-east-1 con DNS opcional
│       └── self-hosted-hetzner/   # Relay barato en Hetzner Falkenstein
└── cloud-init/
    └── raspberry-pi.yml           # Bootstrap directo de SD card para Pi
```

## Quickstart

### Relay en AWS (1 comando)

```sh
cd infra/terraform/examples/single-region-aws
terraform init
terraform apply
```

Output:
```
relay_endpoint = "ec2-xx-xx-xx-xx.compute-1.amazonaws.com"
udp_port       = 9000
ws_url         = "ws://ec2-xx-xx-xx-xx.compute-1.amazonaws.com:9090"
```

### Relay barato en Hetzner

```sh
cd infra/terraform/examples/self-hosted-hetzner
export TF_VAR_hcloud_token="..."     # https://console.hetzner.cloud/projects/.../security/tokens
export TF_VAR_ssh_key_fingerprints='["MD5:xx:xx:..."]'
terraform init
terraform apply
```

~€4/mes total.

### Edge node en Raspberry Pi

1. Flash Raspberry Pi OS Lite (ARM64) en SD card.
2. Antes de bootear:
   ```sh
   sudo cp infra/cloud-init/raspberry-pi.yml /boot/firmware/user-data
   ```
3. Editar `/boot/firmware/user-data` y poner `GS_RELAY_HOST`.
4. Boot. El Pi captura del mic y envía al relay.

## Comparación de proveedores

| Proveedor | Plan mínimo | $/mes | Pros | Cons |
|-----------|-------------|-------|------|------|
| Hetzner   | CX22        | €4-5  | Barato, IPv6, EU/US | Solo 9 ubicaciones |
| DigitalOcean | s-1vcpu-1gb | $6    | Simple, 13 regiones globales | Más caro que Hetzner |
| AWS       | t4g.small   | $12   | Ecosistema completo, Route53, ACM | Más caro, billing complejo |
| Raspberry Pi 4 | hardware once | $0/mes | Self-hosted, edge cases | Limitado por upload casero |

## Operaciones

### Health check

```sh
ssh user@relay-host
curl http://localhost:9100/healthz
# → OK
```

### Métricas Prometheus

Por defecto el endpoint `/metrics` está bind a `127.0.0.1` (sin exposición pública). Para scraping:

```sh
ssh -L 9100:localhost:9100 user@relay-host
curl http://localhost:9100/metrics
```

O añadir un job de Prometheus que use `node_exporter` con TLS para exponerlo.

### Update del binario

```sh
# en la VM:
TAG=v0.2.0-alpha.2
curl -fsSL -o /tmp/gs.tar.gz \
  "https://github.com/angelnereira/gravital-sound/releases/download/$TAG/gs-$TAG-linux-x86_64.tar.gz"
sudo systemctl stop gravital-sound-relay
sudo tar -xzf /tmp/gs.tar.gz -C /tmp
sudo install -m 0755 /tmp/gs-$TAG-linux-x86_64/gs /usr/local/bin/gs-relay
sudo systemctl start gravital-sound-relay
```

## Troubleshooting

| Síntoma | Causa probable | Fix |
|---------|----------------|-----|
| `gs-relay` no arranca | Falta el binario en `/usr/local/bin` | Ver `journalctl -u gravital-sound-relay -e` |
| Cliente no puede conectar UDP | Firewall del proveedor | Ver SG / Cloud Firewall del módulo |
| Latencia > 100 ms LAN | Resampler activo (sample rate mismatch) | Configurar device a 48 kHz nativo |
| OOM tras horas | TTL muy alto, demasiadas sesiones idle | Bajar `session_ttl_secs` en config |

## Pendiente (Track E.2 + E.4)

- Helm chart `gravital-sound-relay` para Kubernetes.
- Stack observability completo (Prometheus + Grafana + Loki) con dashboards prediseñados.
- CLI `gs deploy` que envuelve Terraform para devs sin experiencia.
