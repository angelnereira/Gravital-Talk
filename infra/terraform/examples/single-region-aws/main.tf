# Ejemplo: relay Gravital Sound en AWS us-east-1 con DNS opcional.

terraform {
  required_version = ">= 1.5"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = ">= 5.0"
    }
  }
}

provider "aws" {
  region = "us-east-1"
}

variable "domain" {
  description = "FQDN del relay (opcional). Vacío = sin DNS."
  type        = string
  default     = ""
}

variable "route53_zone_id" {
  description = "Zone ID de Route53 (requerido si domain != \"\")."
  type        = string
  default     = ""
}

module "relay" {
  source = "../../modules/relay-aws"

  region          = "us-east-1"
  name            = "gravital-relay-prod"
  instance_type   = "t4g.small"
  domain          = var.domain
  route53_zone_id = var.route53_zone_id
  release_tag     = "v0.2.0-alpha.1"

  tags = {
    Environment = "production"
    Owner       = "ops@example.com"
  }
}

output "relay_endpoint" {
  value = module.relay.relay_endpoint
}

output "udp_port" {
  value = module.relay.udp_port
}

output "ws_url" {
  value = module.relay.ws_url
}
