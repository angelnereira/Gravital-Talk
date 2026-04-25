# Ejemplo self-hosted: relay barato en Hetzner Cloud (~€4/mes).

terraform {
  required_version = ">= 1.5"
  required_providers {
    hcloud = {
      source  = "hetznercloud/hcloud"
      version = ">= 1.45"
    }
  }
}

variable "hcloud_token" {
  description = "Token API de Hetzner Cloud."
  type        = string
  sensitive   = true
}

variable "ssh_key_fingerprints" {
  description = "Fingerprints SHA256 de SSH keys ya cargadas en Hetzner."
  type        = list(string)
  default     = []
}

provider "hcloud" {
  token = var.hcloud_token
}

module "relay" {
  source = "../../modules/relay-hetzner"

  name        = "gravital-mini"
  location    = "fsn1"  # Falkenstein, DE
  server_type = "cx22"  # 2 vCPU, 4 GB RAM, ~€4/mes
  ssh_keys    = var.ssh_key_fingerprints
}

output "relay_endpoint" {
  value = module.relay.relay_endpoint
}

output "ws_url" {
  value = module.relay.ws_url
}
