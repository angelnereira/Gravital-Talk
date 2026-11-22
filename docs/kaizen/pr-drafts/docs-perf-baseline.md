## Resumen
Publicar un baseline de rendimiento reproducible con fecha, commit, entorno y comandos reales

## Componente afectado
Documentación de rendimiento: `docs/benchmarks.md`

## Tipo de cambio
- [ ] bug
- [ ] enhancement
- [ ] tech-debt
- [x] documentation
- [ ] refactor

## Documentos relacionados
- Issue #44
- `docs/benchmarks.md`
- `AGENTS.md` (regla de regresión >= 10 % con justificación)
- Issue #43 (gate de benchmarks en CI, consume este baseline)

## Plan de prueba
- `cargo bench -p gravital-talk --bench checksum --bench encode_decode -- --save-baseline docs44`
- Verificar que los numeros del documento coinciden con la salida de criterion
- `scripts/kaizen check`

## Impacto en calidad (ratchet)
- Cobertura: igual (no se mide, KZ_COVERAGE_CMD vacio)
- Advertencias: igual (no se mide, KZ_WARNINGS_CMD vacio)
- Deuda (TODO/FIXME): igual (8, linea base 8)

## Criterios de salida que avanza
- `docs/benchmarks.md` incluye fecha, commit del codigo medido, entorno y comando exacto
- Los numeros publicados son medianas reales de criterion, no estimaciones
- La regla de regresion queda explicita y conciliada con AGENTS.md
- Issue #44 cerrado

## Aprendizaje (opcional)
Los numeros anteriores no tenian fecha ni commit asociado, por lo que no servian como baseline comparables. Se midieron de nuevo los dos benches de las primitivas; el loopback y el handshake se dejaron como mediciones historicas no recapturadas.

## Cierre de issues
Closes #44

## Checklist
- [x] Compila
- [x] Pasa tests
- [x] `scripts/kaizen check` en verde
- [x] Documentacion actualizada
- [x] Linea base no empeora
