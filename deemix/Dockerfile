# ================================================================
# Stage 1: Building a Telegram Bot from Source (Rust)
# ================================================================

FROM rust:latest AS telegram_bot-builder
ARG TARGETARCH

RUN apt-get update && apt-get install -y musl-tools && rm -rf /var/lib/apt/lists/*

RUN set -eux; \
    case "${TARGETARCH}" in \
        amd64) rust_target='x86_64-unknown-linux-musl' ;; \
        arm64) rust_target='aarch64-unknown-linux-musl' ;; \
        *) echo >&2 "ERROR: Unsupported TARGETARCH: ${TARGETARCH}"; exit 1 ;; \
    esac; \
    rustup target add "${rust_target}"; \
    echo "${rust_target}" > /rust-target.txt

WORKDIR /telegram_bot-build

COPY Cargo.toml ./
COPY telegram_bot/ ./src/

RUN set -eux; \
    rust_target="$(cat /rust-target.txt)"; \
    cargo build --release --target "${rust_target}"; \
    cp "target/${rust_target}/release/telegram_bot" /telegram_bot

# ================================================================
# Stage 2: Final App Image
# ================================================================

FROM ghcr.io/bambanah/deemix:v4.7.0

ENV S6_BEHAVIOUR_IF_STAGE2_FAILS=2

COPY rootfs/ /

# --- LOCAL PATCH: ingress-compatible WebUI ---
# Replaces source files, then rebuilds both server (tsc) and client (vite).
# npm is installed temporarily (Alpine's nodejs package doesn't include it),
# used for the build, then removed along with all caches to keep the final image clean.
COPY webui-patch/vite.config.mts /app/packages/webui/vite.config.mts
COPY webui-patch/index.html /app/packages/webui/index.html
COPY webui-patch/src/client/main.ts /app/packages/webui/src/client/main.ts
COPY webui-patch/src/client/utils/socket.ts /app/packages/webui/src/client/utils/socket.ts
COPY webui-patch/src/server/main.ts /app/packages/webui/src/server/main.ts
RUN apk add --no-cache npm && \
    cd /app/packages/webui && \
    npm run build && \
    apk del npm && \
    rm -rf \
        /root/.npm \
        /tmp/* \
        /var/cache/apk/* \
        /app/packages/webui/*.tsbuildinfo \
        /app/packages/webui/src/client \
        /app/packages/webui/src/server \
        /app/packages/webui/index.html \
        /app/packages/webui/vite.config.mts
# --- END LOCAL PATCH ---

RUN rm -rf \
    /app/packages/deemix/src \
    /app/packages/deezer-sdk/src \
    /app/packages/cli/src \
    /app/packages/gui/src \
    /app/pnpm-lock.yaml \
    /app/turbo.json \
    /app/pnpm-workspace.yaml

COPY --from=telegram_bot-builder /telegram_bot /usr/local/bin/telegram_bot

RUN chmod +x /custom-cont-init.d/* /usr/local/bin/telegram_bot && \
    find /etc/services.d/ -type f -name "run" -exec chmod +x {} \;

LABEL \
    io.hass.name="Deemix with telegram bot" \
    io.hass.description="High-quality music downloader with a telegram bot: remote, chat-based control for automated downloads." \
    io.hass.type="app" \
    io.hass.url="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.title="Home Assistant App: Deemix with telegram bot" \
    org.opencontainers.image.description="High-quality music downloader with a telegram bot: remote, chat-based control for automated downloads." \
    org.opencontainers.image.source="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.licenses="GPL-3.0"
