## Resumen
Aclarar en la documentación que io-uring/tokio-uring es una idea futura, no una feature implementada

## Componente afectado
Documentación: `docs/transport.md`, `docs/roadmap.md`

## Tipo de cambio
- [ ] bug
- [ ] enhancement
- [ ] tech-debt
- [x] documentation
- [ ] refactor

## Documentos relacionados
- Issue #46
- `docs/transport.md` y `docs/roadmap.md`
- `CHANGELOG.md` ([Unreleased])

## Plan de prueba
- `grep -rn 'io-uring\|tokio-uring' docs --include='*.md'`: toda referencia debe decir explícitamente que no está implementada
- `scripts/kaizen check`

## Impacto en calidad (ratchet)
- Cobertura: igual (no se mide, KZ_COVERAGE_CMD vacio)
- Advertencias: igual (no se mide, KZ_WARNINGS_CMD vacio)
- Deuda (TODO/FIXME): igual (8, linea base 8)

## Criterios de salida que avanza
- Ningún documento describe io-uring/tokio-uring como feature disponible
- Las menciones restantes estan marcadas como idea futura
- Issue #46 cerrado

## Aprendizaje (opcional)
El roadmap arrastraba una deuda documental: transport.md describia una feature condicional que no existe en el codigo.

## Cierre de issues
Closes #46

## Checklist
- [x] Compila
- [x] Pasa tests
- [x] `scripts/kaizen check` en verde
- [x] Documentacion actualizada
- [x] Linea base no empeora
