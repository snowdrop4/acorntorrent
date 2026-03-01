# ------------------------------------------------------------------------------
# Build Stage
# ------------------------------------------------------------------------------
FROM rust:1.93-alpine AS builder

RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    perl \
    make

WORKDIR /build

# Copy the entire project
COPY src            src
COPY Cargo.toml     Cargo.toml
COPY Cargo.lock     Cargo.lock
COPY acornbencode   acornbencode

# Build the project
RUN cargo +nightly build --release

# ------------------------------------------------------------------------------
# Runtime Stage
# ------------------------------------------------------------------------------
FROM alpine:3.19

WORKDIR /app

# Copy binary from builder
COPY --from=builder /build/target/release/acorntorrent /app/acorntorrent

# Create directories for data
RUN mkdir -p /data/downloads /data/torrents

WORKDIR /data

CMD ["/app/acorntorrent"]
