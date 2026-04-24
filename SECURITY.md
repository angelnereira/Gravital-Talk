# Política de seguridad

## Versiones soportadas

Durante el periodo `alpha` y `beta` (`0.1.x`), sólo se parcha la rama `main`. A partir de `0.1.0` final se mantendrá una ventana de soporte de al menos dos versiones menores atrás.

## Cómo reportar una vulnerabilidad

Envía un correo a **security@gravitalsound.dev** con asunto `[SECURITY] <resumen corto>`. Incluye:

1. Descripción clara del problema y el vector de ataque.
2. Versiones afectadas (`gs --version` o hash del commit).
3. Reproducción mínima (preferiblemente un test o script).
4. Impacto esperado y, si es posible, una prueba de concepto.

**No abras issues públicas** con detalles de la vulnerabilidad. Si necesitas cifrado, pide la llave PGP al mismo correo y espera respuesta antes de enviar el reporte.

## SLA de respuesta

- Acuse de recibo: 48 horas hábiles.
- Evaluación inicial (severidad, alcance): 7 días.
- Parche o workaround: dentro de los 30 días siguientes para severidades alta/crítica; 90 días para media/baja.

## Modelo de amenazas

El modelo de amenazas formal está documentado en [`docs/security.md`](docs/security.md). Cubre ataques in-path (MITM), replay, amplificación UDP, exhaustación de recursos y fingerprinting.

## Alcance del reporte

**Dentro de alcance**:
- Crates `gravital-sound-*` y SDKs oficiales en este repositorio.
- El binario `gs` y los ejemplos empaquetados.
- El header FFI y los bindings autogenerados.

**Fuera de alcance**:
- Dependencias transitivas (reporta upstream).
- Despliegues productivos de relays de terceros.
- Uso incorrecto documentado de la API (ej. compartir llaves privadas en logs).

## Reconocimientos

Los reportes válidos se reconocen en `CHANGELOG.md` y, con permiso del reportador, en la página de seguridad. No ofrecemos programa económico por el momento.
