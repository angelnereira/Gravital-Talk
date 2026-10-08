# syntax=docker/dockerfile:1.7
# ─────────────────────────────────────────────────────────────────────
# Gravital Talk — imágenes oficiales (multi-stage, multi-target).
#
# Targets:
#   relay  → daemon `gs-relay` (UDP + WebSocket + Prometheus)
#   cli    → binario `gs` (send/receive/ptt/bench/relay/…)
#
# Build:
#   docker build --target relay -t gravital-talk/relay .
#   docker build --target cli   -t gravital-talk/cli .
#   o usa `docker compose build` (recomendado).

# ── Stage 1: compilación ─────────────────────────────────────────────
FROM rust:1.99-slim-bookworm AS builder

# Features opcionales de cargo (p. ej. `--features grpc` para el plano de
# control gRPC): docker build --build-arg CARGO_FEATURES="--features grpc" .
ARG CARGO_FEATURES=""

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libopus-dev libasound2-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY proto ./proto

# Cache montado: registros y target sobreviven entre builds.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release ${CARGO_FEATURES} \
        -p gravital-talk-cli      --bin gs \
        -p gravital-talk-relay    --bin gs-relay \
    && mkdir -p /out \
    && cp target/release/gs target/release/gs-relay /out/

# ── Stage 2: relay runtime ────────────────────────────────────────────
FROM debian:bookworm-slim AS relay

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --no-create-home --shell /usr/sbin/nologin gravital

COPY --from=builder /out/gs-relay /usr/local/bin/gs-relay

EXPOSE 9000/udp 9090/tcp 9100/tcp

HEALTHCHECK --interval=10s --timeout=3s --retries=6 --start-period=3s \
    CMD curl -fsS http://127.0.0.1:9100/healthz || exit 1

USER gravital
ENV GS_LOG=info
ENTRYPOINT ["/usr/local/bin/gs-relay"]

# ── Stage 3: cli runtime ──────────────────────────────────────────────
FROM debian:bookworm-slim AS cli

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl iproute2 libopus0 libasound2 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10002 --no-create-home --shell /usr/sbin/nologin gravital

COPY --from=builder /out/gs /usr/local/bin/gs

ENV GS_LOG=info
ENTRYPOINT ["/usr/local/bin/gs"]