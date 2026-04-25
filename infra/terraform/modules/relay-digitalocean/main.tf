# Gravital Sound — relay en DigitalOcean
#
# Droplet con firewall DO. Buen balance entre simplicidad y precio
# (~$6/mes el más chico). Más caro que Hetzner pero con mejor presencia
# global (NYC, SFO, LON, FRA, AMS, TOR, BLR, SGP, SYD).

terraform {
  required_version = ">= 1.5"
  required_providers {
    digitalocean = {
      source  = "digitalocean/digitalocean"
      version = ">= 2.40"
    }
  }
}

variable "name" {
  type    = string
  default = "gravital-relay"
}

variable "region" {
  description = "Región DO: nyc1, nyc3, sfo3, lon1, fra1, ams3, tor1, blr1, sgp1, syd1."
  type        = string
  default     = "nyc3"
}

variable "size" {
  description = "Tamaño del droplet. s-1vcpu-1gb = $6/mes."
  type        = string
  default     = "s-1vcpu-1gb"
}

variable "image" {
  type    = string
  default = "debian-12-x64"
}

variable "ssh_key_ids" {
  description = "IDs (numéricos) o fingerprints de SSH keys cargadas en DO."
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

variable "tags" {
  type    = list(string)
  default = ["gravital-sound", "relay"]
}

locals {
  binary_url = "https://github.com/angelnereira/gravital-sound/releases/download/${var.release_tag}/gs-${var.release_tag}-linux-x86_64.tar.gz"
}

resource "digitalocean_droplet" "relay" {
  name     = var.name
  region   = var.region
  size     = var.size
  image    = var.image
  ssh_keys = var.ssh_key_ids
  tags     = var.tags
  user_data = templatefile("${path.module}/../relay-aws/user_data.sh.tftpl", {
    binary_url   = local.binary_url
    udp_port     = var.udp_port
    ws_port      = var.ws_port
    metrics_port = 9100
  })
  monitoring = true
  ipv6       = true
}

resource "digitalocean_firewall" "relay" {
  name        = "${var.name}-fw"
  droplet_ids = [digitalocean_droplet.relay.id]
  tags        = var.tags

  inbound_rule {
    protocol         = "udp"
    port_range       = tostring(var.udp_port)
    source_addresses = ["0.0.0.0/0", "::/0"]
  }

  inbound_rule {
    protocol         = "tcp"
    port_range       = tostring(var.ws_port)
    source_addresses = ["0.0.0.0/0", "::/0"]
  }

  inbound_rule {
    protocol         = "tcp"
    port_range       = "22"
    source_addresses = ["0.0.0.0/0", "::/0"]
  }

  outbound_rule {
    protocol              = "tcp"
    port_range            = "1-65535"
    destination_addresses = ["0.0.0.0/0", "::/0"]
  }

  outbound_rule {
    protocol              = "udp"
    port_range            = "1-65535"
    destination_addresses = ["0.0.0.0/0", "::/0"]
  }

  outbound_rule {
    protocol              = "icmp"
    destination_addresses = ["0.0.0.0/0", "::/0"]
  }
}

output "relay_endpoint" {
  value = digitalocean_droplet.relay.ipv4_address
}

output "ipv6" {
  value = digitalocean_droplet.relay.ipv6_address
}

output "ws_url" {
  value = "ws://${digitalocean_droplet.relay.ipv4_address}:${var.ws_port}"
}

output "udp_port" {
  value = var.udp_port
}

output "droplet_id" {
  value = digitalocean_droplet.relay.id
}
