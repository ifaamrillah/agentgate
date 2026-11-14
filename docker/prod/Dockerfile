# syntax=docker/dockerfile:1.7
# Image production server Agent Gate (M12): satu image untuk web, worker+scheduler, dan migrasi.
#   docker build -f docker/prod/Dockerfile --build-arg BUILD_SHA=$(git rev-parse --short HEAD) \
#     -t agentgate-rs-server:$(git rev-parse --short HEAD) .
# Toolchain sama dengan rust-toolchain.toml dan docker/dev/Dockerfile. Dependency di-cache per
# layer dengan cargo-chef, jadi perubahan kode tidak mengompilasi ulang seluruh dependency.

ARG RUST_IMAGE=rust:1.99.1-bookworm

FROM ${RUST_IMAGE} AS chef
RUN cargo install --locked cargo-chef@0.1.78
WORKDIR /workspace

FROM chef AS planner
COPY . .
# workspace-hack (cargo hakari, .config/hakari.toml) menyatukan feature dependency untuk build dev
# dan tes. Image produksi mengosongkannya supaya feature khusus tes (mis. loco-rs `testing`) tidak
# ikut ter-compile; `cargo metadata` lalu menyesuaikan Cargo.lock (hanya entri workspace-hack).
RUN sed -i '/^### BEGIN HAKARI SECTION/,/^### END HAKARI SECTION/{//!d}' workspace-hack/Cargo.toml \
 && cargo metadata --format-version 1 >/dev/null
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /workspace/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json -p agentgate-server
COPY . .
# Sama dengan stage planner: workspace-hack dikosongkan.
RUN sed -i '/^### BEGIN HAKARI SECTION/,/^### END HAKARI SECTION/{//!d}' workspace-hack/Cargo.toml \
 && cargo metadata --format-version 1 >/dev/null
# CSS produksi (Tailwind standalone, versi + checksum dipin di bin/tailwind).
RUN AG_IN_CONTAINER=1 bin/tailwind --minify
RUN cargo build --release --locked -p agentgate-server \
 && cp target/release/agentgate-server /usr/local/bin/agentgate-server

FROM debian:bookworm-slim AS runtime
ARG BUILD_SHA=unknown
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl tini \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /app --shell /usr/sbin/nologin agentgate
WORKDIR /app
# Aplikasi membaca config/ dan assets/ (view, i18n, ikon, statis) relatif dari working directory.
COPY --from=builder /usr/local/bin/agentgate-server /usr/local/bin/agentgate-server
COPY --from=builder /workspace/server/config ./config
COPY --from=builder /workspace/server/assets ./assets
USER agentgate
ENV LOCO_ENV=production \
    PORT=5150 \
    BUILD_SHA=${BUILD_SHA}
LABEL org.opencontainers.image.title="agentgate-server" \
      org.opencontainers.image.revision="${BUILD_SHA}"
EXPOSE 5150
# Liveness; readiness (database + queue) di /_readiness untuk reverse proxy/orkestrator.
HEALTHCHECK --interval=15s --timeout=3s --start-period=20s --retries=3 \
  CMD curl -fsS "http://127.0.0.1:${PORT}/_health" >/dev/null || exit 1
ENTRYPOINT ["tini", "--", "agentgate-server"]
CMD ["start", "--no-banner"]
