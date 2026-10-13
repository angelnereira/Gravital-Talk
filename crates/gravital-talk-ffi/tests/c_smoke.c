/* Smoke test del header C. Compila con:
 *   cc -I include -c tests/c_smoke.c -o /tmp/c_smoke.o
 * Sólo valida que el header sea válido C; no lincamos la libreria aqui. */
#include <stdio.h>
#include "gravital_talk.h"

int main(void) {
    GsConfig cfg;
    GsStatus st = gs_config_default(&cfg);
    if (st != GS_OK) {
        fprintf(stderr, "gs_config_default failed: %d\n", (int)st);
        return 1;
    }
    printf("Gravital Talk C ABI v%u, protocol v%u, runtime v%s\n",
           gs_abi_version(),
           gs_protocol_version(),
           gs_version());
    printf("Default config: %u Hz, %u ch, %u ms, MTU=%u\n",
           cfg.sample_rate, cfg.channels, cfg.frame_duration_ms, cfg.mtu);
    /* El audio nativo es opcional segun el build, pero sus simbolos existen
     * siempre: con la sesion sin audio, las estadisticas salen a cero. */
    GsSessionHandle *session = NULL;
    st = gs_session_create(&cfg, "127.0.0.1", 0, &session);
    if (st != GS_OK) {
        fprintf(stderr, "gs_session_create failed: %d\n", (int)st);
        return 1;
    }
    GsAudioStats audio;
    st = gs_audio_stats(session, &audio);
    if (st != GS_OK || audio.rx_frames != 0) {
        fprintf(stderr, "gs_audio_stats failed: %d\n", (int)st);
        return 1;
    }
    printf("Audio nativo: %s\n", gs_audio_supported() ? "si" : "no");

    /* Un perfil de voz se puede elegir antes de conectar; uno que no existe se rechaza. */
    if (gs_session_set_voice_profile(session, GS_VOICE_CLEAR) != GS_OK ||
        gs_session_set_voice_profile(session, 200) != GS_ERR_INVALID_ARGUMENT) {
        fprintf(stderr, "gs_session_set_voice_profile no se comporta como dice\n");
        return 1;
    }

    /* Sin servidor de encuentro: quitarlo es valido y no toca la red. */
    st = gs_session_set_rendezvous(session, NULL, 0);
    if (st != GS_OK) {
        fprintf(stderr, "gs_session_set_rendezvous failed: %d\n", (int)st);
        return 1;
    }
    /* Una candidata mal escrita se rechaza antes de intentar conectar. */
    if (gs_session_connect_any(session, "no-es-una-direccion") != GS_ERR_INVALID_ARGUMENT) {
        fprintf(stderr, "gs_session_connect_any acepto una candidata invalida\n");
        return 1;
    }
    gs_session_destroy(session);

    /* Nulo es válido: deja de anotar fallos. */
    if (gs_crash_log_set_path(NULL) != GS_OK) {
        return 1;
    }

    char ip[64];
    if (gs_discover_local_addr(ip, sizeof ip) == GS_OK) {
        printf("IP local: %s\n", ip);
    }
    return gs_ping();
}
