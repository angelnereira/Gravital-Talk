# Gravital Sound — edge-node generic module
#
# Module-agnostic: produce un user_data válido para CUALQUIER VPS o
# bare-metal con cloud-init y systemd. El consumidor lo pasa al
# campo `user_data` de su recurso de Compute (AWS EC2, Hetzner Server,
# DO Droplet, OCI Compute, GCP GCE, etc.).
#
# Uso típico — pequeño VPS o Raspberry Pi en una red doméstica:
#
#   module "edge" {
#     source         = "./infra/terraform/modules/edge-node"
#     relay_host     = "relay.example.com"
#     relay_udp_port = 9000
#     codec          = "opus"
#   }
#
#   # Pasar el output a tu provider de compute:
#   resource "hcloud_server" "edge" {
#     # ...
#     user_data = module.edge.user_data
#   }

terraform {
  required_version = ">= 1.5"
}

variable "relay_host" {
  description = "FQDN o IP del relay al que el edge node se conecta."
  type        = string
}

variable "relay_udp_port" {
  type    = number
  default = 9000
}

variable "device" {
  description = "Nombre del audio device a capturar. 'default' = device por defecto del sistema."
  type        = string
  default     = "default"
}

variable "codec" {
  description = "Codec a usar: pcm o opus."
  type        = string
  default     = "opus"
  validation {
    condition     = contains(["pcm", "opus"], var.codec)
    error_message = "codec debe ser 'pcm' o 'opus'."
  }
}

variable "release_tag" {
  type    = string
  default = "v0.2.0-alpha.1"
}

variable "architecture" {
  description = "Arquitectura del binario a descargar. aarch64 (Pi/ARM VPS) o x86_64."
  type        = string
  default     = "aarch64"
  validation {
    condition     = contains(["aarch64", "x86_64"], var.architecture)
    error_message = "architecture debe ser 'aarch64' o 'x86_64'."
  }
}

locals {
  binary_url = "https://github.com/angelnereira/gravital-sound/releases/download/${var.release_tag}/gs-${var.release_tag}-linux-${var.architecture}.tar.gz"
}

# Outputs cloud-init listo para pegar en cualquier provider.
output "user_data" {
  description = "user_data cloud-init para aprovisionar el edge node."
  value = templatefile("${path.module}/edge_user_data.sh.tftpl", {
    relay_host     = var.relay_host
    relay_udp_port = var.relay_udp_port
    device         = var.device
    codec          = var.codec
    binary_url     = local.binary_url
  })
}
