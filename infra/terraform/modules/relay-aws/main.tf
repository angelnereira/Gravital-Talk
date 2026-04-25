# Gravital Sound — relay en AWS EC2
#
# Despliega una instancia EC2 (ARM64 por defecto) con `gs-relay` corriendo
# como systemd unit, security group con UDP/WS abiertos a internet, y
# opcionalmente un record A en Route53.

terraform {
  required_version = ">= 1.5"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = ">= 5.0"
    }
  }
}

# ---- Datos ----

data "aws_vpc" "default" {
  default = true
}

data "aws_subnets" "default" {
  filter {
    name   = "vpc-id"
    values = [data.aws_vpc.default.id]
  }
}

data "aws_ami" "debian_arm64" {
  count       = var.ami_id == "" ? 1 : 0
  most_recent = true
  owners      = ["136693071363"] # Debian official

  filter {
    name   = "name"
    values = ["debian-12-arm64-*"]
  }
  filter {
    name   = "architecture"
    values = ["arm64"]
  }
  filter {
    name   = "virtualization-type"
    values = ["hvm"]
  }
  filter {
    name   = "root-device-type"
    values = ["ebs"]
  }
}

locals {
  ami           = var.ami_id != "" ? var.ami_id : data.aws_ami.debian_arm64[0].id
  base_tags     = merge({ Project = "gravital-sound", Component = "relay" }, var.tags)
  binary_url    = "https://github.com/angelnereira/gravital-sound/releases/download/${var.release_tag}/gs-${var.release_tag}-linux-aarch64.tar.gz"
  has_dns       = var.domain != "" && var.route53_zone_id != ""
  ssh_enabled   = length(var.allowed_ssh_cidrs) > 0 && var.key_name != ""
}

# ---- Security group ----

resource "aws_security_group" "relay" {
  name_prefix = "${var.name}-"
  description = "Gravital Sound relay: UDP audio + WS + (opt) metrics"
  vpc_id      = data.aws_vpc.default.id
  tags        = merge(local.base_tags, { Name = var.name })

  egress {
    description = "All egress"
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  ingress {
    description = "Gravital Sound audio (UDP)"
    from_port   = var.udp_port
    to_port     = var.udp_port
    protocol    = "udp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  ingress {
    description = "Gravital Sound WebSocket bridge"
    from_port   = var.ws_port
    to_port     = var.ws_port
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  dynamic "ingress" {
    for_each = var.expose_metrics_externally ? [1] : []
    content {
      description = "Gravital Sound metrics (Prometheus)"
      from_port   = var.metrics_port
      to_port     = var.metrics_port
      protocol    = "tcp"
      cidr_blocks = ["0.0.0.0/0"]
    }
  }

  dynamic "ingress" {
    for_each = local.ssh_enabled ? [1] : []
    content {
      description = "SSH"
      from_port   = 22
      to_port     = 22
      protocol    = "tcp"
      cidr_blocks = var.allowed_ssh_cidrs
    }
  }
}

# ---- EC2 instance ----

resource "aws_instance" "relay" {
  ami                    = local.ami
  instance_type          = var.instance_type
  vpc_security_group_ids = [aws_security_group.relay.id]
  subnet_id              = data.aws_subnets.default.ids[0]
  key_name               = var.key_name == "" ? null : var.key_name

  associate_public_ip_address = true
  user_data                   = templatefile("${path.module}/user_data.sh.tftpl", {
    binary_url   = local.binary_url
    udp_port     = var.udp_port
    ws_port      = var.ws_port
    metrics_port = var.metrics_port
  })
  user_data_replace_on_change = true

  metadata_options {
    http_tokens = "required"
  }

  root_block_device {
    volume_size = 20
    volume_type = "gp3"
    encrypted   = true
  }

  tags = merge(local.base_tags, { Name = var.name })
}

# ---- DNS opcional ----

resource "aws_route53_record" "relay" {
  count   = local.has_dns ? 1 : 0
  zone_id = var.route53_zone_id
  name    = var.domain
  type    = "A"
  ttl     = 60
  records = [aws_instance.relay.public_ip]
}
