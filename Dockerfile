# ---- Base build image ----
FROM rust:slim-bookworm AS chef
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# cargo-chef splits the build in two, so the (slow) dependency compilation is
# cached independently of the (fast-changing) workspace sources.
RUN cargo install cargo-chef --locked

# ---- Dependency recipe ----
# Reduces the workspace to just its dependency graph.
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---- Build stage ----
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Builds only the dependencies. This layer is reused on every build that does
# not change Cargo.toml / Cargo.lock, so editing Rust code no longer recompiles
# the whole dependency tree.
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN cargo build --release --workspace

# ---- Runtime stage ----
FROM debian:bookworm-slim

# Needed for outbound HTTPS (Mongo Atlas, JWKS endpoint).
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --system --create-home appuser
WORKDIR /app
COPY --from=builder /app/target/release/poolnhl_app ./server

USER appuser
EXPOSE 8000

# config/release.json is read from the working directory at startup and is
# NOT baked into the image. Mount the non-secret defaults in at runtime:
#   docker run -v $(pwd)/config:/app/config:ro -p 8000:8000 <image>
# Override/inject secrets per-deploy via env vars instead of the file, e.g.:
#   docker run -e APP_DATABASE__URI=mongodb+srv://... <image>
CMD ["./server"]
