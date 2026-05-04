# Gravital Talk — relay productivo
# Imagen multi-stage para minimizar tamaño final.

# ---------- Stage 1: build ----------
FROM rust:1.78-slim-bookworm AS builder
WORKDIR /build

# Cache de dependencias: copiar manifests primero.
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates/gravital-talk-core/Cargo.toml         crates/gravital-talk-core/Cargo.toml
COPY crates/gravital-talk-metrics/Cargo.toml      crates/gravital-talk-metrics/Cargo.toml
COPY crates/gravital-talk-transport/Cargo.toml    crates/gravital-talk-transport/Cargo.toml
COPY crates/gravital-talk-codec/Cargo.toml        crates/gravital-talk-codec/Cargo.toml
COPY crates/gravital-talk-io/Cargo.toml           crates/gravital-talk-io/Cargo.toml
COPY crates/gravital-talk/Cargo.toml              crates/gravital-talk/Cargo.toml
COPY crates/gravital-talk-ffi/Cargo.toml          crates/gravital-talk-ffi/Cargo.toml
COPY crates/gravital-talk-cli/Cargo.toml          crates/gravital-talk-cli/Cargo.toml
COPY crates/gravital-talk-relay/Cargo.toml        crates/gravital-talk-relay/Cargo.toml

# Copiar el código real y compilar el binario release.
COPY crates ./crates

RUN cargo build --release -p gravital-talk-relay --bin gs-relay

# ---------- Stage 2: runtime ----------
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --no-create-home --shell /usr/sbin/nologin gravital

COPY --from=builder /build/target/release/gs-relay /usr/local/bin/gs-relay

# Puertos por defecto:
# - 9000/udp  → tráfico de audio (datagramas)
# - 9090/tcp  → WebSocket bridge
# - 9100/tcp  → métricas Prometheus + /healthz
EXPOSE 9000/udp 9090/tcp 9100/tcp

USER gravital
ENV GS_LOG=info

ENTRYPOINT ["/usr/local/bin/gs-relay"]
CMD []
