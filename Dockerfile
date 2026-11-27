# `-bookworm`, not the bare `latest-rust-1`, and this is load-bearing. That
# tag is rolling: it moved to a trixie base (glibc 2.38) while the runtime
# stage below stayed on bookworm-slim (glibc 2.36). The build keeps
# succeeding; the binary then dies at startup with
#   libc.so.6: version `GLIBC_2.38' not found
# which is exactly how one caldav-rs replica ended up in CrashLoopBackOff on
# an image whose sibling, built before the tag moved, ran fine.
FROM lukemathwalker/cargo-chef:latest-rust-1-bookworm AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --no-default-features --features ssr
COPY . .
RUN cargo build --release --bin caldav-portal --no-default-features --features ssr

# The hydration bundle, built separately on purpose: `hydrate` pulls in
# browser-only APIs that do not exist for this stage's host target, so the two
# cannot be one cargo invocation.
FROM chef AS wasm-builder
RUN rustup target add wasm32-unknown-unknown
# Pinned to the same version as the `wasm-bindgen` crate in Cargo.toml. The
# generated glue and this CLI share an unstable schema and refuse to work
# unless they match to the patch -- the failure reads
# "rust Wasm file schema version / this binary schema version" and has
# nothing to do with the code. The two numbers change together.
RUN cargo install wasm-bindgen-cli --version 0.2.129 --locked
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --target wasm32-unknown-unknown --no-default-features --features hydrate
COPY . .
RUN cargo build --release --lib --target wasm32-unknown-unknown --no-default-features --features hydrate
# Content-hash the bundle name so a changed build gets a new URL and no proxy
# or browser serves yesterday's wasm against today's server. LEPTOS_OUTPUT_NAME
# is read at startup so the SSR shell points at this build's hash.
RUN HASH=$(sha256sum target/wasm32-unknown-unknown/release/caldav_portal.wasm | cut -c1-12) && \
    echo "caldav-portal-$HASH" > /app/output-name.txt && \
    wasm-bindgen target/wasm32-unknown-unknown/release/caldav_portal.wasm \
      --out-dir pkg --out-name "caldav-portal-$HASH" --target web --no-typescript

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/caldav-portal /usr/local/bin/caldav-portal
COPY --from=wasm-builder /app/pkg /pkg
COPY --from=wasm-builder /app/output-name.txt /output-name.txt
WORKDIR /
EXPOSE 8091
ENTRYPOINT ["/bin/sh", "-c", "export LEPTOS_OUTPUT_NAME=$(cat /output-name.txt) && exec caldav-portal"]
