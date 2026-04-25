output "instance_id" {
  description = "ID de la instancia EC2."
  value       = aws_instance.relay.id
}

output "public_ip" {
  description = "IP pública asignada a la instancia."
  value       = aws_instance.relay.public_ip
}

output "public_dns" {
  description = "DNS público AWS de la instancia."
  value       = aws_instance.relay.public_dns
}

output "fqdn" {
  description = "FQDN del relay si se configuró Route53; vacío si no."
  value       = local.has_dns ? aws_route53_record.relay[0].fqdn : ""
}

output "relay_endpoint" {
  description = "Endpoint canónico para clientes del relay (FQDN si hay DNS, IP pública si no)."
  value       = local.has_dns ? aws_route53_record.relay[0].fqdn : aws_instance.relay.public_ip
}

output "udp_port" {
  description = "Puerto UDP del relay."
  value       = var.udp_port
}

output "ws_url" {
  description = "URL ws:// para clientes WebSocket."
  value       = "ws://${local.has_dns ? aws_route53_record.relay[0].fqdn : aws_instance.relay.public_ip}:${var.ws_port}"
}
