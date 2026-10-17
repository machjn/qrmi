# syntax=docker/dockerfile:1

# ---------------------------------------------------------------------------
# Build stage
# ---------------------------------------------------------------------------
FROM rust:1.91-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev musl-tools \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace manifests first to cache dependency downloads.
# rust-toolchain.toml must be present before any cargo/rustup invocation
# so the pinned toolchain is activated.
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY dependencies/ dependencies/
COPY operator/Cargo.toml operator/Cargo.toml

# Install the musl target here (rather than in rust-toolchain.toml) so
# only the container build pays the cost, not every contributor's rustup.
# The target follows the platform being built (TARGETARCH is set by
# docker/podman), so `--platform linux/amd64` or `linux/arm64` both work.
ARG TARGETARCH
RUN case "$TARGETARCH" in \
      amd64) echo x86_64-unknown-linux-musl ;; \
      arm64) echo aarch64-unknown-linux-musl ;; \
      *) echo "unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac > /rust-target \
    && rustup target add "$(cat /rust-target)"

# cbindgen.toml is required by the qrmi crate's build.rs
COPY cbindgen.toml .

# Fake source so cargo can fetch deps before copying real source.
RUN mkdir -p operator/src && echo 'fn main(){}' > operator/src/main.rs \
    && mkdir -p src && touch src/lib.rs

RUN cargo build --release --target "$(cat /rust-target)" -p qrmi-operator --locked 2>/dev/null || true

# Now copy real source and rebuild (only changed crates recompile).
COPY src/ src/
COPY build.rs .
COPY operator/src/ operator/src/

RUN cargo build --release --target "$(cat /rust-target)" -p qrmi-operator --locked \
    && cp "target/$(cat /rust-target)/release/qrmi-operator" /build/qrmi-operator

# ---------------------------------------------------------------------------
# Runtime stage — static musl binary needs no glibc; scratch is minimal
# ---------------------------------------------------------------------------
FROM gcr.io/distroless/static-debian12:nonroot

COPY --from=builder /build/qrmi-operator /qrmi-operator

ENTRYPOINT ["/qrmi-operator"]
