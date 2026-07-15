# ---- Build stage ----
FROM rust:1.78-slim-bookworm AS builder
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

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
