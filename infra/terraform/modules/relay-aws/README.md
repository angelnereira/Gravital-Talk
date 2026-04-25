# Módulo Terraform — `relay-aws`

Despliega un **Gravital Sound relay** en una instancia EC2 (ARM64 por defecto, ~$12/mes en `t4g.small`), con security group y opcionalmente un record A en Route53.

## Uso mínimo

```hcl
module "relay" {
  source = "github.com/angelnereira/gravital-sound//infra/terraform/modules/relay-aws"

  region = "us-east-1"
  name   = "my-relay"
}

output "relay_ip" {
  value = module.relay.public_ip
}
```

## Uso con DNS

```hcl
module "relay" {
  source = "github.com/angelnereira/gravital-sound//infra/terraform/modules/relay-aws"

  region          = "us-east-1"
  name            = "prod-relay"
  domain          = "relay.example.com"
  route53_zone_id = "Z123ABCXYZ"
}
```

## Variables principales

| Variable | Default | Descripción |
|----------|---------|-------------|
| `region` | `us-east-1` | Región AWS. |
| `instance_type` | `t4g.small` | Tipo de instancia. ARM64 recomendado. |
| `name` | `gravital-relay` | Prefijo de nombre + tag `Name`. |
| `udp_port` | `9000` | Puerto UDP del relay. |
| `ws_port` | `9090` | Puerto WebSocket. |
| `metrics_port` | `9100` | Puerto de `/metrics` y `/healthz`. Cerrado al exterior por defecto. |
| `expose_metrics_externally` | `false` | Si `true`, abre `metrics_port` a internet. |
| `domain` | `""` | FQDN para record A en Route53 (opcional). |
| `route53_zone_id` | `""` | Zone ID de Route53 (requerido si `domain` no vacío). |
| `key_name` | `""` | Key pair EC2 para SSH. |
| `allowed_ssh_cidrs` | `[]` | CIDRs autorizados a SSH. |
| `release_tag` | `v0.2.0-alpha.1` | Tag del binario `gs-relay` a descargar. |

## Outputs

- `relay_endpoint` — FQDN si hay DNS, IP pública si no.
- `udp_port` / `ws_url` — para configurar clientes.
- `instance_id`, `public_ip`, `public_dns`, `fqdn`.

## Detalles operacionales

- La instancia usa **Debian 12 ARM64** por defecto, IMDSv2 obligatorio, root EBS gp3 cifrado.
- Bootstrap vía `user_data` cloud-init: descarga el tar.gz del Release de GitHub, instala `gs-relay` en `/usr/local/bin`, lo corre como systemd unit con usuario sin shell.
- Firewall UFW configurado: deny incoming + allow puertos del relay + (opcional) SSH.
- El puerto de `/metrics` está bind a `127.0.0.1` por defecto. Para scraping desde Prometheus externo, usar SSH tunnel o un sidecar (ej. `node_exporter` con TLS).

## Coste estimado

- `t4g.small` (2 vCPU ARM, 2 GiB) en us-east-1: ~$12/mes on-demand.
- 20 GiB gp3 EBS cifrado: ~$1.6/mes.
- Tráfico saliente: $0.09/GB (los primeros 100 GB/mes son gratis).
- **Total típico**: ~$15-20/mes para un relay con tráfico moderado.

## Cleanup

```sh
terraform destroy
```
