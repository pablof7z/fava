# syntax=docker/dockerfile:1@sha256:ecfaec9ed6d810b56388c508f4121597bfbba70d41a6dfeee4d8cad5f295fc32
ARG SOURCE_IMAGE
FROM ${SOURCE_IMAGE} AS pinned_source
SHELL ["/bin/bash", "-euo", "pipefail", "-c"]

FROM ${SOURCE_IMAGE} AS buildkit_probe
SHELL ["/bin/bash", "-euo", "pipefail", "-c"]
ARG FAVA_SOURCE_MANIFEST_SHA256
USER 65532:65532
RUN --mount=type=bind,from=pinned_source,source=/attestation,target=/probe,rw \
    test "$(id -u)" = 65532 \
    && test "$(ulimit -u)" -le 512 \
    && printf x >> /probe/source.manifest \
    && test "$(tail -c 1 /probe/source.manifest)" = x
RUN --mount=type=bind,from=pinned_source,source=/attestation,target=/probe,ro \
    test "$(id -u)" = 65532 \
    && test "$(ulimit -u)" -le 512 \
    && printf '%s  %s\n' "${FAVA_SOURCE_MANIFEST_SHA256}" /probe/source.manifest | sha256sum -c - \
    && python3 -c 'import errno; p="/probe/source.manifest"; exec("try:\n f=open(p, \"ab\")\nexcept OSError as e:\n assert e.errno == errno.EROFS\nelse:\n f.close()\n raise SystemExit(86)")'

FROM ${SOURCE_IMAGE} AS compiler
SHELL ["/bin/bash", "-euo", "pipefail", "-c"]
ARG FAVA_REVISION
ARG FAVA_TREE
ARG FAVA_SOURCE_TREE_SHA256
ARG FAVA_SOURCE_MANIFEST_SHA256
ARG FAVA_SOURCE_IMAGE_SHA256
ARG FAVA_RUST_BASE_IMAGE_SHA256
USER root
RUN mkdir /output && chown 65532:65532 /output
USER 65532:65532
RUN --network=none \
    --mount=type=bind,from=pinned_source,source=/source,target=/source,ro \
    --mount=type=bind,from=pinned_source,source=/attestation,target=/attestation,ro \
    --mount=type=tmpfs,target=/target,size=4294967296,uid=65532,gid=65532,mode=0700 \
    test "$(id -u)" = 65532 \
    && test "$(ulimit -u)" -le 512 \
    && cd /source \
    && mkdir -p /target/tmp \
    && CARGO_INCREMENTAL=0 \
       CARGO_TARGET_DIR=/target \
       TMPDIR=/target/tmp \
       FAVA_CANARY_PINNED_BUILD=1 \
       FAVA_BUILD_REVISION=${FAVA_REVISION} \
       FAVA_BUILD_TREE=${FAVA_TREE} \
       FAVA_BUILD_SOURCE_TREE_SHA256=${FAVA_SOURCE_TREE_SHA256} \
       FAVA_BUILD_SOURCE_MANIFEST_SHA256=${FAVA_SOURCE_MANIFEST_SHA256} \
       FAVA_BUILD_SOURCE_IMAGE_SHA256=${FAVA_SOURCE_IMAGE_SHA256} \
       FAVA_BUILD_RUST_BASE_IMAGE_SHA256=${FAVA_RUST_BASE_IMAGE_SHA256} \
       cargo build --frozen --offline --release \
         --manifest-path apps/canary/Cargo.toml --bin canary \
    && install -m 0500 /target/release/canary /output/canary

FROM scratch
COPY --from=compiler --chmod=0500 /output/canary /canary
