# Loop SWEEP: combatir la entropía

Elige UN ítem de la lista, el que lleve más tiempo sin ejecutarse según el diario:

1. Dependencias: actualizaciones de parche y menores con tests verdes; marca las mayores como issue.
2. Deuda: convierte TODO/FIXME/HACK con más de 90 días en issues con contexto, o elimínalos si ya no aplican.
3. Desviación documental: compara README y docs con el comportamiento real del código y corrige.
4. Pruebas inestables: detecta tests flaky (repite la suite 3 veces) y abre issues o estabilízalos sin saltarlos.
5. Código muerto y duplicado: elimina lo comprobadamente sin uso, con tests verdes.
6. Aptitud arquitectónica: verifica las reglas de docs/kaizen/fitness.md (dependencias entre módulos, tamaño máximo, ciclos) y reporta violaciones.
7. Seguridad y licencias: ejecuta la auditoría del proyecto (audit/deny/equivalente) y abre issues.
8. Descriptores huérfanos: elimina docs/kaizen/pr-drafts/*.md cuya rama ya no exista o cuyo PR esté fusionado.
9. Retro: ejecuta `scripts/kaizen retro` y redacta docs/kaizen/retros/ con hasta 3 mejoras propuestas (decide el humano).
10. Línea base: si la calidad medida mejoró, prepara el cambio de `.kaizen/baseline.json` como PR (requiere revisión humana).

Reglas: diff pequeño, tests verdes, `scripts/kaizen check` verde, diario al final.
