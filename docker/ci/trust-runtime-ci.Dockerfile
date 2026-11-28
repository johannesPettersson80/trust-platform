# truST project CI image (specification 22, "Project CI image"): `trust-runtime` and `trust-dev` of the truST release
# that TRUST_VERSION names, checked against the release's SHA256SUMS, with the release's toolchain installed, so that
# a PLC project's pipeline builds, validates and tests the project without Rust and without a truST checkout.
#
# Build, from this folder:
#   docker build --build-arg TRUST_VERSION=0.44.1 -f trust-runtime-ci.Dockerfile -t trust-runtime-ci:0.44.1 .
# Run, in a project's folder:
#   docker run --rm -v "$PWD":/workspace trust-runtime-ci:0.44.1 trust-runtime build --project . --ci
#   docker run --rm -v "$PWD":/workspace trust-runtime-ci:0.44.1 trust-runtime validate --project . --ci
#   docker run --rm -v "$PWD":/workspace trust-runtime-ci:0.44.1 trust-dev test --project . --ci --output junit
FROM debian:bookworm-slim@sha256:e67eeead736ff7bbe258ef088bff683635c561b1fcfefad0806c1df4ab49ce0e

ARG TRUST_VERSION=0.44.1
# Set by BuildKit for the platform the image is built for.
ARG TARGETARCH

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

# The release archive for the image's platform, checked against its line in SHA256SUMS before it is unpacked; the
# sums name each archive with a folder prefix, so the line is found by the archive's own name.
RUN set -eu; \
    case "${TARGETARCH:-amd64}" in \
        amd64) platform=linux-x64 ;; \
        arm64) platform=linux-arm64 ;; \
        *) echo "truST releases no tools for ${TARGETARCH}" >&2; exit 1 ;; \
    esac; \
    archive="trust-runtime-${platform}.tar.gz"; \
    base="https://github.com/johannesPettersson80/trust-platform/releases/download/v${TRUST_VERSION}"; \
    cd /tmp; \
    curl -fsSLO "${base}/${archive}"; \
    curl -fsSLO "${base}/SHA256SUMS"; \
    expected=$(awk -v name="${archive}" '{ n = split($2, part, "/"); if (part[n] == name) print $1 }' SHA256SUMS); \
    test -n "${expected}"; \
    echo "${expected}  ${archive}" | sha256sum --check --strict -; \
    mkdir -p /opt/trust; \
    tar -xzf "${archive}" -C /opt/trust; \
    rm "${archive}" SHA256SUMS

ENV PATH=/opt/trust:${PATH}

# The toolchain of the same release (specification 44 §4.6), in truST's data folder of the image's user, root.
RUN trust-runtime toolchain install

WORKDIR /workspace
