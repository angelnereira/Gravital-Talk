/* Smoke test del header C. Compila con:
 *   cc -I include -c tests/c_smoke.c -o /tmp/c_smoke.o
 * Sólo valida que el header sea válido C; no lincamos la libreria aqui. */
#include <stdio.h>
#include "gravital_sound.h"

int main(void) {
    GsConfig cfg;
    GsStatus st = gs_config_default(&cfg);
    if (st != GS_OK) {
        fprintf(stderr, "gs_config_default failed: %d\n", (int)st);
        return 1;
    }
    printf("Gravital Sound C ABI v%u, protocol v%u, runtime v%s\n",
           gs_abi_version(),
           gs_protocol_version(),
           gs_version());
    printf("Default config: %u Hz, %u ch, %u ms, MTU=%u\n",
           cfg.sample_rate, cfg.channels, cfg.frame_duration_ms, cfg.mtu);
    return gs_ping();
}
