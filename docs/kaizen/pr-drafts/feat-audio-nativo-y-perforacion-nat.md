## Resumen
Instalar gobernanza Kaizen: reglas canonicas, CLI, hooks, descriptores, linea base y compuertas

## Componente afectado
Proceso de desarrollo de todo el repo: .kaizen/, scripts/kaizen, .githooks/, .github/, docs/kaizen/, AGENTS.md, CHANGELOG.md y documentacion existente

## Tipo de cambio
- [ ] bug
- [x] enhancement
- [ ] tech-debt
- [x] documentation
- [ ] refactor

## Documentos relacionados
- `.kaizen/RULES.md` (fuente unica de reglas)
- `AGENTS.md` (bloque Kaizen y precedencia de la convencion de commits)
- `CHANGELOG.md` (entrada en [Unreleased])
- `docs/adr/` y `docs/*.md` (sustitucion de marcas graficas de aceptacion/rechazo)

## Plan de prueba
- `scripts/kaizen init`
- `scripts/kaizen check --only descriptors`
- `scripts/kaizen check --only policy`
- `scripts/kaizen check --only ratchet`
- `scripts/kaizen check`
- `scripts/kaizen baseline` y `scripts/kaizen metrics`
- `.githooks/commit-msg` con un resumen de 300 caracteres (debe rechazarlo)

## Impacto en calidad (ratchet)
- Cobertura: igual (no se mide, KZ_COVERAGE_CMD vacio)
- Advertencias: igual (no se mide, KZ_WARNINGS_CMD vacio)
- Deuda (TODO/FIXME): igual (8, linea base 8)

## Criterios de salida que avanza
- `scripts/kaizen check` en verde en esta rama
- `core.hooksPath=.githooks` activo y hooks ejecutables
- `.kaizen/baseline.json` y `docs/kaizen/data/metrics.jsonl` generados

## Aprendizaje (opcional)
El conteo de lineas con `app/` incluia binarios de app/build/ (cientos de MB), por eso los globos se acotaron a `crates app/lib`. `check_policy` imprimia avisos en stderr sin marcar FAIL; corregido para que la politica se cumpla.

## Cierre de issues
Closes: none

## Checklist
- [x] Compila
- [x] Pasa tests
- [x] `scripts/kaizen check` en verde
- [x] Documentacion actualizada
- [x] Linea base no empeora
