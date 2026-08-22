ARG RUST_IMAGE
FROM ${RUST_IMAGE}

ARG RUST_IMAGE
ARG FAVA_REVISION
ARG FAVA_TREE
ARG FAVA_SOURCE_MANIFEST_SHA256
SHELL ["/bin/bash", "-euo", "pipefail", "-c"]

LABEL org.opencontainers.image.revision="${FAVA_REVISION}" \
      org.fava.source-tree="${FAVA_TREE}" \
      org.fava.source-manifest-sha256="${FAVA_SOURCE_MANIFEST_SHA256}" \
      org.fava.rust-base-image="${RUST_IMAGE}"

WORKDIR /source
COPY --chown=65532:65532 source/ /source/
COPY --chown=65532:65532 control/source.manifest /attestation/source.manifest

RUN mkdir -p /home/fava \
    && chown 65532:65532 /home/fava \
    && chown -R 65532:65532 /usr/local/cargo

USER 65532:65532
ENV HOME=/home/fava

RUN test "$(id -u)" = 65532 \
    && test "$(ulimit -u)" -le 512 \
    && /source/apps/canary/tools/verify-pinned-source.sh \
      /source /attestation/source.manifest "${FAVA_SOURCE_MANIFEST_SHA256}" \
    && cargo fetch --locked --manifest-path /source/apps/canary/Cargo.toml

ENV CARGO_INCREMENTAL=0 \
    CARGO_TARGET_DIR=/target \
    TMPDIR=/target/tmp
