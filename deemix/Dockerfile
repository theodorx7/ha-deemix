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
# Stage 2: Deemix Builder (rebuild + cleanup)
# ================================================================

FROM ghcr.io/bambanah/deemix:v4.7.0 AS deemix-builder

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
        /app/packages/cli \
        /app/packages/webui/src \
        /app/packages/webui/public \
        /app/packages/webui/index.html \
        /app/packages/webui/vite.config.mts \
        /app/packages/webui/postcss.config.cjs \
        /app/packages/webui/tailwind.config.mjs \
        /app/packages/webui/tsconfig.json \
        /app/packages/webui/tsconfig.config.json \
        /app/packages/webui/vitest.config.ts \
        /app/packages/webui/README.md \
        /app/packages/webui/CHANGELOG.md \
        /app/packages/deemix/src \
        /app/packages/deemix/README.md \
        /app/packages/deemix/CHANGELOG.md \
        /app/packages/deezer-sdk/src \
        /app/packages/deezer-sdk/README.md \
        /app/packages/deezer-sdk/CHANGELOG.md \
        /app/package.json \
        /app/pnpm-lock.yaml \
        /app/pnpm-workspace.yaml \
        /app/turbo.json \
        /app/.npmrc && \
    find /app/packages -type f \
        \( -name "*.d.ts" \
        -o -name "*.d.ts.map" \
        -o -name "*.map" \
        -o -name "*.tsbuildinfo" \) \
        -delete

# ================================================================
# Stage 3: Final Image (clean base + only what's needed)
# ================================================================

FROM ghcr.io/linuxserver/baseimage-alpine:3.24

# Install Node.js (like in the original bambanah/deemix【turn5fetch0】)
RUN apk add --no-cache nodejs>=24.0.0

# Copy the entire cleaned /app (preserves pnpm symlinks)
COPY --from=deemix-builder /app /app

# S6 structure deemix (services, init scripts, usr)
COPY --from=deemix-builder /custom-cont-init.d /custom-cont-init.d
COPY --from=deemix-builder /custom-services.d /custom-services.d
COPY --from=deemix-builder /usr /usr

# ENV from bambanah/deemix【turn5fetch0】
ENV NODE_ENV=production
ENV DEEMIX_DATA_DIR=/config/
ENV DEEMIX_MUSIC_DIR=/downloads/
ENV S6_BEHAVIOUR_IF_STAGE2_FAILS=2

COPY rootfs/ /

COPY --from=telegram_bot-builder /telegram_bot /usr/local/bin/telegram_bot

RUN chmod +x /custom-cont-init.d/* /usr/local/bin/telegram_bot && \
    find /etc/services.d/ -type f -name "run" -exec chmod +x {} \; && \
    find /custom-services.d/ -type f -name "run" -exec chmod +x {} \;

EXPOSE 6595

LABEL \
    io.hass.name="Deemix with telegram bot" \
    io.hass.description="High-quality music downloader with a telegram bot: remote, chat-based control for automated downloads." \
    io.hass.type="app" \
    io.hass.url="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.title="Home Assistant App: Deemix with telegram bot" \
    org.opencontainers.image.description="High-quality music downloader with a telegram bot: remote, chat-based control for automated downloads." \
    org.opencontainers.image.source="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.licenses="GPL-3.0"
