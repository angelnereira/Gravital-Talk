---
description: Ejecuta una iteracion Kaizen (dev | ops | sweep | research | product | status)
---
Si $ARGUMENTS es "status", ejecuta `scripts/kaizen-loop status` y resume.
En otro caso, lee `.kaizen/RULES.md`, `.kaizen/loop/LOOP.md` y `docs/kaizen/loop/prompts/$ARGUMENTS.md`, y ejecuta UNA iteracion en la rama actual (no en la rama por defecto). No hagas push. Deja la entrada de diario.
