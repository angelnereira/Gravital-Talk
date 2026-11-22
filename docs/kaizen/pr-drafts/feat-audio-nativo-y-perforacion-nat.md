## Resumen
Instalar gobernanza Kaizen y el ciclo continuo Kaizen Loop: reglas, CLI, hooks, compuertas, orquestador y telemetria

## Componente afectado
Proceso de desarrollo de todo el repo: .kaizen/, scripts/kaizen*, .githooks/, .github/, .opencode/, docs/kaizen/, AGENTS.md, CHANGELOG.md, opencode.json, .gitignore y documentacion existente

## Tipo de cambio
- [ ] bug
- [x] enhancement
- [ ] tech-debt
- [x] documentation
- [ ] refactor

## Documentos relacionados
- `.kaizen/RULES.md` y `.kaizen/loop/LOOP.md` (fuentes unicas de reglas)
- `docs/kaizen/loop/prompts/` (los cinco loops)
- `docs/kaizen/telemetry/EVENT_SCHEMA.md` y `PRIVACY.md`
- `AGENTS.md` (bloques Kaizen y Kaizen Loop; precedencia de la convencion de commits)
- `CHANGELOG.md` (entrada en [Unreleased])
- `docs/adr/` y `docs/*.md` (sustitucion de marcas graficas de aceptacion/rechazo)

## Plan de prueba
- `scripts/kaizen check`
- `scripts/kaizen init` y `scripts/kaizen-loop init`
- `scripts/kaizen-loop status`
- `KL_DRY_RUN=1 scripts/kaizen-loop once dev`
- Comprobar que con `.kaizen/loop/PAUSE` presente no se lanza agente
- Disyuntor con `echo 3 > .kaizen/loop/state/fails` (issue #41 cerrado tras verificar)
- `scripts/kaizen-loop classify` sobre un workflow protegido y sobre un doc
- `python3 scripts/kaizen-insights.py` con JSONL sintetico de 10 eventos y 3 sesiones
- `.githooks/commit-msg` con un resumen de 300 caracteres (debe rechazarlo)

## Impacto en calidad (ratchet)
- Cobertura: igual (no se mide, KZ_COVERAGE_CMD vacio)
- Advertencias: igual (no se mide, KZ_WARNINGS_CMD vacio)
- Deuda (TODO/FIXME): igual (8, linea base 8)

## Criterios de salida que avanza
- `scripts/kaizen check` en verde en esta rama
- `core.hooksPath=.githooks` activo y hooks ejecutables
- `.kaizen/baseline.json` y `docs/kaizen/data/metrics.jsonl` generados
- Loop instalado con `KL_AUTONOMY=1` y `.kaizen/loop/PAUSE` presente

## Aprendizaje (opcional)
El conteo de lineas con `app/` incluia binarios de app/build/ (cientos de MB), por eso los globos se acotaron a `crates app/lib`. `check_policy` imprimia avisos en stderr sin marcar FAIL; corregido. Los permisos del loop se pusieron en un agente `kaizen-loop` aparte y el orquestador lo selecciona con `--agent`, para no restringir las sesiones interactivas. El saneador de telemetria no elimina texto de instrucciones: la defensa son L-5, el aviso de datos no confiables en los issues y el permiso `rm *` denegado al agente.

## Cierre de issues
Closes: none

## Checklist
- [x] Compila
- [x] Pasa tests
- [x] `scripts/kaizen check` en verde
- [x] Documentacion actualizada
- [x] Linea base no empeora
