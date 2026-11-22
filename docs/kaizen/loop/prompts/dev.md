# Loop DEV: una tarea del backlog aprobado

1. Lee .kaizen/RULES.md, .kaizen/loop/LOOP.md y las últimas 10 entradas de docs/kaizen/journal/.
2. Elige UN issue abierto con etiqueta `kaizen:ready` (mayor prioridad, luego el más antiguo): `gh issue list --label kaizen:ready --json number,title,labels`. Sin gh: el primer ítem sin marcar de docs/kaizen/BACKLOG.md. Si no hay trabajo, escribe la entrada de diario "sin trabajo" y termina.
3. Revisa los criterios de aceptación. Si son ambiguos o no verificables, comenta en el issue con preguntas concretas, etiquétalo `needs-human` y termina.
4. Escribe primero la prueba (bug: prueba que falla; feature: prueba de aceptación).
5. Implementa lo mínimo para pasarla. Ejecuta formato, lint y tests.
6. Actualiza documentación afectada y rellena el descriptor (parte "Impacto en calidad" incluida) con `Closes #N`.
7. `scripts/kaizen check` debe terminar en verde. Haz commit según K-3.
8. Escribe la entrada de diario (L-13) y termina. No abras el PR: lo hace el orquestador.
