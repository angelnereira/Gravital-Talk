# Gravital Sound — relay en Hetzner Cloud
#
# Hetzner es la opción más barata para self-hosting:
# - CX22 (formerly CX11): ~€4-5/mes con 2 vCPU + 4 GB RAM
# - Excelente conectividad europea, buena globalmente
# - Cloud Firewall incluido sin costo extra

terraform {
  required_version = ">= 1.5"
  required_providers {
    hcloud = {
      source  = "hetznercloud/hcloud"
      version = ">= 1.45"
    }
  }
}

variable "name" {
  type    = string
  default = "gravital-relay"
}

variable "location" {
  description = "Datacenter Hetzner: nbg1 (Núremberg), fsn1 (Falkenstein), hel1 (Helsinki), ash (Ashburn US), hil (Hillsboro US)."
  type        = string
  default     = "nbg1"
}

variable "server_type" {
  description = "Tipo de servidor. cx22 es el más barato (~€4/mes)."
  type        = string
  default     = "cx22"
}

variable "image" {
  description = "Imagen base. Default Debian 12."
  type        = string
  default     = "debian-12"
}

variable "ssh_keys" {
  description = "Lista de fingerprints o IDs de SSH keys ya cargadas en Hetzner Cloud."
  type        = list(string)
  default     = []
}

variable "udp_port" {
  type    = number
  default = 9000
}

variable "ws_port" {
  type    = number
  default = 9090
}

variable "release_tag" {
  type    = string
  default = "v0.2.0-alpha.1"
}

variable "labels" {
  type    = map(string)
  default = { project = "gravital-sound", component = "relay" }
}

locals {
  binary_url = "https://github.com/angelnereira/gravital-sound/releases/download/${var.release_tag}/gs-${var.release_tag}-linux-x86_64.tar.gz"
}

resource "hcloud_firewall" "relay" {
  name   = "${var.name}-fw"
  labels = var.labels

  rule {
    direction = "in"
    protocol  = "udp"
    port      = tostring(var.udp_port)
    source_ips = ["0.0.0.0/0", "::/0"]
  }

  rule {
    direction = "in"
    protocol  = "tcp"
    port      = tostring(var.ws_port)
    source_ips = ["0.0.0.0/0", "::/0"]
  }

  rule {
    direction = "in"
    protocol  = "tcp"
    port      = "22"
    source_ips = ["0.0.0.0/0", "::/0"]
  }
}

resource "hcloud_server" "relay" {
  name        = var.name
  image       = var.image
  server_type = var.server_type
  location    = var.location
  ssh_keys    = var.ssh_keys
  firewall_ids = [hcloud_firewall.relay.id]
  labels      = var.labels

  user_data = templatefile("${path.module}/../relay-aws/user_data.sh.tftpl", {
    binary_url   = local.binary_url
    udp_port     = var.udp_port
    ws_port      = var.ws_port
    metrics_port = 9100
  })

  public_net {
    ipv4_enabled = true
    ipv6_enabled = true
  }
}

output "relay_endpoint" {
  value = hcloud_server.relay.ipv4_address
}

output "ipv6" {
  value = hcloud_server.relay.ipv6_address
}

output "ws_url" {
  value = "ws://${hcloud_server.relay.ipv4_address}:${var.ws_port}"
}

output "udp_port" {
  value = var.udp_port
}
