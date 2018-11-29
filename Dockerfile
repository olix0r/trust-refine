# Proxy build and runtime
#
# This is intended **DEVELOPMENT ONLY**, i.e. so that proxy developers can
# easily test the proxy in the context of the larger `linkerd2` project.
#
# When PROXY_UNOPTIMIZED is set and not empty, unoptimized rust artifacts are produced.
# This reduces build time and produces binaries with debug symbols, at the expense of
# runtime performance.

ARG RUST_IMAGE=rust:1.30.1
ARG RUNTIME_IMAGE=debian:stretch-slim

## Builds the proxy as incrementally as possible.
FROM $RUST_IMAGE as build

WORKDIR /usr/src/trust-refine

# Fetch external dependencies.
RUN mkdir -p src && touch src/main.rs
COPY Cargo.toml Cargo.lock ./
RUN cargo fetch --locked

# Build the proxy binary using the already-built dependencies.
COPY src src
RUN cargo build --frozen --release


## Install the proxy binary into the base runtime image.
FROM $RUNTIME_IMAGE as runtime
WORKDIR /
COPY --from=build /usr/src/trust-refine/target/release/trust-refine ./trust-refine
ENV LINKERD2_PROXY_LOG=warn,trust_refine=info
ENTRYPOINT ["/trust-refine"]
