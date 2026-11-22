# Esquema de eventos (JSON Lines, un evento por línea)

Campos (todos saneados en el origen; ver PRIVACY.md):

- ts: marca de tiempo ISO 8601 UTC
- session: identificador aleatorio y rotativo (nunca un ID de usuario ni de dispositivo estable)
- version: versión de la aplicación
- env: prod | staging | dev
- type: error | crash | perf | usage
- name: nombre estable del evento (p. ej. "checkout.submit", "parse.config")
- fingerprint: hash estable del error (tipo + frames del stack sin rutas locales)
- message: texto saneado, máximo 200 caracteres
- stack_top: hasta 5 frames, sin rutas absolutas ni datos de usuario
- duration_ms: solo type=perf
- attrs: objeto con claves de una lista permitida (p. ej. os, locale, feature_flag)

Ejemplo:
{"ts":"2026-11-21T10:00:00Z","session":"a91f...","version":"1.4.2","env":"prod","type":"error","name":"parse.config","fingerprint":"9c1e44ab","message":"unexpected token at line 3","stack_top":["parse_config:88","load:41"],"attrs":{"os":"linux"}}
