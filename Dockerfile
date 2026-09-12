# ==============================================================================
# Stage 1: Build WebConsole (Svelte 5 SPA)
# ==============================================================================
FROM node:22-alpine AS web-builder
WORKDIR /app/web
RUN corepack enable && corepack prepare pnpm@latest --activate
COPY web/package.json web/pnpm-lock.yaml* web/pnpm-workspace.yaml* ./
RUN pnpm install --frozen-lockfile
COPY web/ ./
RUN pnpm build

# ==============================================================================
# Stage 2: Build Rust Binary (StorageNode + CLI + Embedded Assets)
# ==============================================================================
FROM rust:1.94-bookworm AS rust-builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src/ src/
COPY migrations/ migrations/
COPY --from=web-builder /app/web/dist/ web/dist/
RUN cargo build --release

# ==============================================================================
# Stage 3: Minimal Runtime Container
# ==============================================================================
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates sqlite3 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=rust-builder /app/target/release/r2drive /usr/local/bin/r2drive
EXPOSE 8080
VOLUME ["/data", "/etc/r2drive"]
ENV R2DRIVE_CONFIG=/etc/r2drive/config.yaml
ENTRYPOINT ["r2drive", "serve"]
