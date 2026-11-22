## Resumen
Publicar los CLI de macOS y Windows en el release builds con nombres fijos

## Componente afectado
CI / distribución: `.github/workflows/build-outputs.yml`, `outputs/README.md`, `docs/roadmap.md`

## Tipo de cambio
- [ ] bug
- [x] enhancement
- [ ] tech-debt
- [ ] documentation
- [ ] refactor

## Documentos relacionados
- Issue #45
- `outputs/README.md` (tabla de descargas)
- `docs/roadmap.md` (deuda técnica, ahora marcada como hecha)
- `CHANGELOG.md` ([Unreleased])

## Plan de prueba
- `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/build-outputs.yml'))"`
- Verificar que los jobs macos-cli y windows-cli tienen el paso de upload al release con nombre fijo
- `scripts/kaizen check`

## Impacto en calidad (ratchet)
- Cobertura: igual (no se mide, KZ_COVERAGE_CMD vacio)
- Advertencias: igual (no se mide, KZ_WARNINGS_CMD vacio)
- Deuda (TODO/FIXME): igual (8, linea base 8)

## Criterios de salida que avanza
- macOS publica `gravital-talk-latest-macos-x86_64.tar.gz` y `...-aarch64.tar.gz`
- Windows publica `gravital-talk-latest-windows-x64.zip`
- Los nombres son fijos y se usan con `--clobber`, igual que el APK
- `outputs/README.md` documenta los cinco enlaces de descarga
- Issue #45 cerrado

## Aprendizaje (opcional)
El workflow ya compilaba y subia artifactos de macOS y Windows, pero el release `builds` solo tenia Android y Linux: la deuda del roadmap estaba desactualizada. Faltaba la ultima milla de publicacion con nombre fijo.

## Cierre de issues
Closes #45

## Checklist
- [x] Compila
- [x] Pasa tests
- [x] `scripts/kaizen check` en verde
- [x] Documentacion actualizada
- [x] Linea base no empeora
