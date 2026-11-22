## Resumen
Validar el YAML de los workflows dentro de kaizen check

## Componente afectado
Gobernanza / CI: `scripts/kaizen`, `.kaizen/config.env`

## Tipo de cambio
- [ ] bug
- [x] enhancement
- [ ] tech-debt
- [ ] documentation
- [ ] refactor

## Documentos relacionados
- Issue #56
- `.kaizen/config.env` (dependencia opcional documentada)
- `AGENTS.md` (bloque Kaizen)

## Plan de prueba
- `scripts/kaizen check --only workflows` con los workflows actuales -> en verde
- Anadir un workflow con YAML invalido (staged) -> el check debe fallar; borrar despues
- `scripts/kaizen check` completo

## Impacto en calidad (ratchet)
- Cobertura: igual (no se mide, KZ_COVERAGE_CMD vacio)
- Advertencias: igual (no se mide, KZ_WARNINGS_CMD vacio)
- Deuda (TODO/FIXME): igual (8, linea base 8)

## Criterios de salida que avanza
- `kaizen check` falla si algun workflow tiene YAML invalido
- La dependencia PyYAML es opcional y se documenta; si falta se reporta, no se finge
- Issue #56 cerrado

## Aprendizaje (opcional)
La compuerta nueva usa archivos versionados (`git ls-files`), asi que no se puede evadir con un fichero sin staged.

## Cierre de issues
Closes #56

## Checklist
- [x] Compila
- [x] Pasa tests
- [x] `scripts/kaizen check` en verde
- [x] Documentacion actualizada
- [x] Linea base no empeora
