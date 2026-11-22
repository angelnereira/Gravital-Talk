## Resumen
Comparar benchmarks base vs head en el mismo runner y fallar si hay regresion >= 10 %

## Componente afectado
CI / rendimiento: `.github/workflows/bench-gate.yml`, `scripts/bench-compare.py`, `docs/benchmarks.md`

## Tipo de cambio
- [ ] bug
- [x] enhancement
- [ ] tech-debt
- [ ] documentation
- [ ] refactor

## Documentos relacionados
- Issue #43
- `docs/benchmarks.md` (sección Quality gates para CI)
- `AGENTS.md` (regla de regresion >= 10 %)
- Issue #44 (baseline de referencia humana, no la usa el gate)

## Plan de prueba
- Local: `cargo bench -p gravital-talk --bench checksum --bench encode_decode -- --save-baseline base` y luego `--save-baseline head` sobre el mismo codigo; `python3 scripts/bench-compare.py --base base --head head --threshold 10` debe dar 0 %
- Negativo: copiar `target/criterion` a un temporal, inflar una mediana head un 12 % y verificar que el comparador falla con exit 1
- `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/bench-gate.yml'))"`
- `scripts/kaizen check`

## Impacto en calidad (ratchet)
- Cobertura: igual (no se mide, KZ_COVERAGE_CMD vacio)
- Advertencias: igual (no se mide, KZ_WARNINGS_CMD vacio)
- Deuda (TODO/FIXME): igual (8, linea base 8)

## Criterios de salida que avanza
- El workflow corre benches en el commit base y en el head del PR en el mismo runner
- `bench-compare.py` lee `median.point_estimate` de criterion, imprime la tabla y falla con exit 1 si la regresion es >= 10 %
- Verificado el caso positivo (0 %) y el negativo (12 % inflado) en local
- YAML del workflow valido
- `docs/benchmarks.md` describe el gate y aclara que la baseline versionada es solo referencia humana

## Aprendizaje (opcional)
Un baseline versionado medido en una maquina concreta daria falsos positivos en runners de GitHub con otro hardware. Medir base y head en el mismo runner elimina esa variable; la baseline de #44 sigue sirviendo para humanos, no para el gate.

## Cierre de issues
Closes #43

## Checklist
- [x] Compila
- [x] Pasa tests
- [x] `scripts/kaizen check` en verde
- [x] Documentacion actualizada
- [x] Linea base no empeora
