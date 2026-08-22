ARG RUST_IMAGE
FROM ${RUST_IMAGE}

ARG RUST_IMAGE
ARG FAVA_REVISION
ARG FAVA_TREE
ARG FAVA_SOURCE_MANIFEST_SHA256

LABEL org.opencontainers.image.revision="${FAVA_REVISION}" \
      org.fava.source-tree="${FAVA_TREE}" \
      org.fava.source-manifest-sha256="${FAVA_SOURCE_MANIFEST_SHA256}" \
      org.fava.rust-base-image="${RUST_IMAGE}"

WORKDIR /source
COPY source/ /source/
COPY control/source.manifest /attestation/source.manifest

RUN /source/apps/canary/tools/verify-pinned-source.sh \
      /source /attestation/source.manifest "${FAVA_SOURCE_MANIFEST_SHA256}" \
    && cargo fetch --locked --manifest-path /source/apps/canary/Cargo.toml

ENV CARGO_INCREMENTAL=0 \
    CARGO_TARGET_DIR=/target \
    TMPDIR=/target/tmp
