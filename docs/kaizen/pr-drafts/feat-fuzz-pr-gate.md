## Resumen
Integrar el smoke de fuzzing como gate de PR con presupuesto corto por target

## Componente afectado
CI / seguridad: `.github/workflows/fuzz.yml`, `fuzz/Cargo.toml`, `fuzz/fuzz_targets/fuzz_floor_payload.rs`, `.gitignore`

## Tipo de cambio
- [ ] bug
- [x] enhancement
- [ ] tech-debt
- [ ] documentation
- [ ] refactor

## Documentos relacionados
- Issue #42
- `docs/roadmap.md` (deuda técnica: fuzzing como gate de PR; ahora marcada como hecha)
- `CHANGELOG.md` ([Unreleased])
- Issues #43/#44/#45/#46/#51/#52/#53 referenciados desde el roadmap

## Plan de prueba
- `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/fuzz.yml'))"` -> 3 disparadores, `FUZZ_SECONDS=15` en PR
- `cargo +nightly fuzz run <target> -- -max_total_time=5` para los 3 targets -> sin crashes
- `scripts/kaizen check` -> en verde

## Impacto en calidad (ratchet)
- Cobertura: igual (no se mide, KZ_COVERAGE_CMD vacio)
- Advertencias: igual (no se mide, KZ_WARNINGS_CMD vacio)
- Deuda (TODO/FIXME): igual (8, linea base 8)

## Criterios de salida que avanza
- El workflow se dispara en `pull_request` contra `main` (verificado con YAML parseado)
- En PRs el presupuesto es 15 s por target; en schedule/manual 60 s o el valor del dispatch
- Un crash falla el job y sube los artifacts (paso ya existente, `if: failure()`)
- Concurrencia cancela ejecuciones obsoletas del mismo PR
- Smoke local: fuzz_floor_payload 2.138.434 ejecuciones, fuzz_message_type 4.148.212, fuzz_packet_parse 2.322.746; los 3 sin crashes

## Aprendizaje (opcional)
Al verificar aparecieron dos defectos que hacian que el fuzzing ya existente no funcionara en absoluto: `fuzz/Cargo.toml` no tenia tabla `[workspace]` (Cargo lo rechazaba por pertenecer al workspace raiz) y `fuzz_floor_payload` llamaba a `is_transmitting()`, metodo sustituido por `is_granted()`. El gate usa el mismo workflow que el smoke semanal para no duplicar pasos; solo cambia el presupuesto segun el evento. Docker no estaba disponible en local, asi que la verificacion se hizo con cargo-fuzz directamente.

## Cierre de issues
Closes #42

## Checklist
- [x] Compila
- [x] Pasa tests
- [x] `scripts/kaizen check` en verde
- [x] Documentacion actualizada
- [x] Linea base no empeora
