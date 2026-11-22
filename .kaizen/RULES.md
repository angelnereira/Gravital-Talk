# Reglas Kaizen

## K-1 Fuente de verdad
La documentación en `docs/` y los ADR mandan sobre el código. Si el código y la documentación discrepan, se corrige uno de los dos en el mismo cambio. Nada de decisiones de arquitectura sin ADR.

## K-2 Un tablero de issues
Cada issue lleva tipo (bug, enhancement, tech-debt, documentation), área, prioridad y criterios de aceptación verificables con pasos concretos. No se cierra sin evidencia: test, comando reproducible o CI verde. Si queda bloqueado, permanece abierto con etiqueta `blocked` y motivo. Se cierra en cuanto se verifica.

## K-3 Commits
- Una línea de resumen de máximo 256 caracteres, en imperativo.
- Incluye `Closes #NNN`, `Fixes #NNN` o `Resolves #NNN` cuando corresponda.
- Un commit = un cambio lógico, que compila y pasa tests.
- Políticas opcionales (ver `.kaizen/config.env`): sin emojis, sin atribución a herramientas de IA.

## K-4 Flujo de trabajo (PDCA)
1. Planear: leer el issue, definir criterios de salida y plan de prueba. Crear rama y descriptor `docs/kaizen/pr-drafts/<rama-aplanada>.md` (`/` se convierte en `-`).
2. Hacer: implementar en cambios pequeños. Actualizar el descriptor en el mismo commit que cambie el alcance.
3. Verificar: ejecutar `scripts/kaizen check`. Todo debe pasar.
4. Actuar: abrir PR (el cuerpo se copia del descriptor). Al fusionar, borrar el descriptor en esa misma PR. Registrar lo aprendido si hubo sorpresas.

Sin descriptor no hay PR. Los commits directos a la rama por defecto solo se permiten con excepción explícita del responsable del repo, y entonces cada commit lleva su keyword de cierre.

## K-5 Ratchet de calidad
`.kaizen/baseline.json` guarda las métricas de calidad. Un cambio no puede empeorarlas: cobertura no baja, advertencias de lint no suben, deuda marcada (TODO/FIXME/HACK) no sube sin issue asociado. Para mover la línea base hacia arriba: `scripts/kaizen baseline`. Para aceptar un empeoramiento: ADR o nota en el descriptor con justificación y aprobación humana. El agente nunca relaja la línea base por su cuenta.

## K-6 Recolección de datos
`scripts/kaizen metrics` agrega una línea a `docs/kaizen/data/metrics.jsonl` por ejecución (fecha, commit, tamaño, deuda, cobertura, advertencias, issues abiertos). Se ejecuta en CI al fusionar a la rama por defecto y antes de cada retrospectiva. Los datos se usan para decidir, no para vigilar a personas.

## K-7 Documentación viva
Todo cambio de comportamiento actualiza README, documentación de usuario, ADR o runbook afectado y CHANGELOG. El descriptor lista los documentos tocados. Un cambio sin documentación no está terminado.

## K-8 Operaciones
- Cada servicio u operación recurrente tiene runbook en `docs/kaizen/runbooks/`.
- Todo incidente genera postmortem sin culpables en `docs/kaizen/postmortems/` en un máximo de 5 días, con causa raíz (5 porqués), línea de tiempo y acciones preventivas convertidas en issues.
- Las alertas ruidosas se corrigen o se eliminan; no se ignoran.

## K-9 Retrospectiva
Cada 2 semanas (o por hito): `scripts/kaizen retro` genera el resumen con métricas. El equipo decide como máximo 3 mejoras, cada una con dueño, issue y fecha. En la siguiente retrospectiva se revisa si funcionaron.

## K-10 Límites del agente
- No fabricar evidencia: si un test no se ejecutó, se dice.
- No cerrar issues sin verificación.
- No saltarse hooks (`--no-verify`) ni desactivar CI para que algo pase.
- Ante conflicto entre una regla y una petición del usuario, informar al usuario y pedir decisión.
- La descripción del PR la redacta una persona o el agente en el descriptor; la automatización solo copia y valida.
