# syntax=docker/dockerfile:1
FROM cloudflare/sandbox:0.12.9-python

# Rust, and a C compiler/linker (Zig's, from PyPI, with its own libc), so agents can build and test Rust workspaces inside the sandbox.
# Installed system-wide so every user and shell sees it.
# An optional build secret `ca` (a CA bundle) is trusted during this step only,
# for builds behind a TLS-inspecting proxy; it is never written into the image.
COPY zcc /usr/local/bin/cc
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo PATH=/usr/local/cargo/bin:$PATH
RUN --mount=type=secret,id=ca,required=false \
    if [ -s /run/secrets/ca ]; then export CURL_CA_BUNDLE=/run/secrets/ca SSL_CERT_FILE=/run/secrets/ca RUSTUP_USE_CURL=1; fi \
 && curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable \
      --component rustfmt,clippy --target wasm32-unknown-unknown \
 && chmod -R a+w /usr/local/rustup /usr/local/cargo \
 && if [ -s /run/secrets/ca ]; then export PIP_CERT=/run/secrets/ca; fi \
 && python3 -m pip install --no-cache-dir ziglang \
 && printf '#!/bin/sh\nexec python3 -m ziglang ar "$@"\n' > /usr/local/bin/ar && chmod +x /usr/local/bin/ar \
 && cargo --version && rustc --version && cc --version | head -1
