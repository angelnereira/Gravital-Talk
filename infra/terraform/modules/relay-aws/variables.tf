variable "name" {
  description = "Prefijo de nombre para los recursos creados (etiqueta + DNS)."
  type        = string
  default     = "gravital-relay"
}

variable "region" {
  description = "Región AWS donde desplegar el relay."
  type        = string
  default     = "us-east-1"
}

variable "instance_type" {
  description = "Tipo de instancia EC2. ARM64 (t4g.*) es ~25% más barato y rinde igual o mejor."
  type        = string
  default     = "t4g.small"
}

variable "ami_id" {
  description = "AMI para la instancia. Si vacío, se usa el AMI Debian 12 ARM64 oficial más reciente."
  type        = string
  default     = ""
}

variable "key_name" {
  description = "Nombre del key pair EC2 para acceso SSH (opcional). Vacío = sin SSH."
  type        = string
  default     = ""
}

variable "allowed_ssh_cidrs" {
  description = "CIDRs autorizados a SSH al puerto 22. Default vacío (sin SSH)."
  type        = list(string)
  default     = []
}

variable "udp_port" {
  description = "Puerto UDP en que el relay acepta tráfico de audio."
  type        = number
  default     = 9000
}

variable "ws_port" {
  description = "Puerto TCP para el bridge WebSocket."
  type        = number
  default     = 9090
}

variable "metrics_port" {
  description = "Puerto TCP para /metrics y /healthz. Cerrado al exterior por defecto."
  type        = number
  default     = 9100
}

variable "expose_metrics_externally" {
  description = "Si true, abre metrics_port a internet (no recomendado en prod)."
  type        = bool
  default     = false
}

variable "domain" {
  description = "Dominio FQDN para el relay (ej. relay.example.com). Vacío = sin DNS."
  type        = string
  default     = ""
}

variable "route53_zone_id" {
  description = "Hosted zone ID de Route53 para crear el record A. Requerido si domain != \"\"."
  type        = string
  default     = ""
}

variable "release_tag" {
  description = "Tag del binario gs-relay a descargar de GitHub Releases (ej. v0.2.0-alpha.1)."
  type        = string
  default     = "v0.2.0-alpha.1"
}

variable "tags" {
  description = "Tags adicionales aplicados a todos los recursos."
  type        = map(string)
  default     = {}
}
