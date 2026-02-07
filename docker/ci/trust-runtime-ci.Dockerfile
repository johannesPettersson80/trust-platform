FROM rust:1.84-bookworm

ENV CARGO_TERM_COLOR=always
WORKDIR /workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Deterministic CI base image for trust-runtime project pipelines.
# Usage:
#   docker build -f docker/ci/trust-runtime-ci.Dockerfile -t trust-runtime-ci:local .
#   docker run --rm -v "$PWD":/workspace -w /workspace trust-runtime-ci:local \
#     cargo run -p trust-runtime --bin trust-runtime -- test --project . --ci --output junit
