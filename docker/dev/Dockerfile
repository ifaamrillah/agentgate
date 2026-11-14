# Image toolchain development. Semua perintah Rust dijalankan di sini lewat bin/*;
# host cukup punya Docker.
FROM rust:1.99.1-bookworm

ARG UID=1000
ARG GID=1000

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl gnupg lsb-release \
 && install -d /usr/share/postgresql-common/pgdg \
 && curl -fsSL https://www.postgresql.org/media/keys/ACCC4CF8.asc -o /usr/share/postgresql-common/pgdg/apt.postgresql.org.asc \
 && echo "deb [signed-by=/usr/share/postgresql-common/pgdg/apt.postgresql.org.asc] https://apt.postgresql.org/pub/repos/apt bookworm-pgdg main" > /etc/apt/sources.list.d/pgdg.list \
 && apt-get update \
 && apt-get install -y --no-install-recommends postgresql-client-18 python3 \
 && rm -rf /var/lib/apt/lists/*

# Tool dev prebuilt lewat cargo-binstall (tanpa compile dari source).
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/v1.26.2/install-from-binstall-release.sh | bash \
 && cargo binstall -y --locked \
      watchexec-cli@2.8.0 sccache@0.19.0 cargo-deny@0.20.2 cargo-insta@1.49.0 cargo-chef@0.1.78 loco@1.3.1 \
      cargo-nextest@0.9.152 cargo-hakari@0.9.39 \
 && rustup component add rustfmt clippy \
 && chmod -R a+rwX /usr/local/cargo /usr/local/rustup

# sea-orm-cli butuh feature postgres-vector (tanpanya kolom vector ter-generate sebagai String).
RUN cargo install --locked sea-orm-cli@2.0.4 --no-default-features \
      --features cli,codegen,runtime-tokio-rustls,sqlx-postgres,postgres-vector \
 && rm -rf /usr/local/cargo/registry \
 && chmod -R a+rwX /usr/local/cargo

RUN groupadd -g "${GID}" dev 2>/dev/null || true \
 && useradd -m -u "${UID}" -g "${GID}" -s /bin/bash dev \
 && mkdir -p /workspace/target /home/dev/.cache/sccache /home/dev/.cache/agentgate \
      /usr/local/cargo/registry /usr/local/cargo/git \
 && chown -R "${UID}:${GID}" /workspace /home/dev /usr/local/cargo/registry /usr/local/cargo/git

USER dev
# CARGO_BUILD_JOBS juga membatasi thread codegen rustc (jobserver), bukan hanya jumlah crate
# paralel: 4 membuat compile ulang crate server memakai 4 core saja. 8 menjaga RAM tetap aman di
# laptop 16 GB (link binary tes ~250 MB); compose meneruskan AG_CARGO_JOBS untuk menimpanya.
ENV CARGO_HOME=/usr/local/cargo \
    CARGO_TARGET_DIR=/workspace/target \
    CARGO_BUILD_JOBS=8 \
    RUSTC_WRAPPER=sccache \
    SCCACHE_DIR=/home/dev/.cache/sccache
WORKDIR /workspace
CMD ["sleep", "infinity"]
