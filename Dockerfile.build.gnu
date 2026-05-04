ARG RUST_TARGET=x86_64-unknown-linux-gnu

FROM rust:bookworm AS builder

ARG RUST_TARGET
ARG RUST_TOOLCHAIN=nightly-2026-01-01

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    cmake \
    perl \
    pkg-config \
    libx11-dev \
    libxcursor-dev \
    libxrandr-dev \
    libxi-dev \
    libxinerama-dev \
    libwayland-dev \
    libxkbcommon-dev \
    libfontconfig1-dev \
    libdbus-1-dev \
    libgtk-3-dev \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN rustup toolchain install "${RUST_TOOLCHAIN}" && \
    rustup target add --toolchain "${RUST_TOOLCHAIN}" "${RUST_TARGET}"

WORKDIR /usr/src/odd-box

COPY . .
COPY --from=cruma-sdk . ./cruma-sdk/

RUN cargo build --profile dist --target "${RUST_TARGET}" -p odd-box

FROM scratch AS export
ARG RUST_TARGET
COPY --from=builder \
    /usr/src/odd-box/target/${RUST_TARGET}/dist/odd-box \
    /odd-box
