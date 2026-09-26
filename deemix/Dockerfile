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
# Stage 2: Deemix Client Rebuild
# ================================================================

FROM ghcr.io/bambanah/deemix:v4.7.0 AS deemix-builder

COPY webui-patch/vite.config.mts /app/packages/webui/vite.config.mts
COPY webui-patch/index.html /app/packages/webui/index.html
COPY webui-patch/src/client/main.ts /app/packages/webui/src/client/main.ts
COPY webui-patch/src/client/utils/socket.ts /app/packages/webui/src/client/utils/socket.ts
# COPY webui-patch/src/client/styles/css/global.css /app/packages/webui/src/client/styles/css/global.css

RUN cd /app/packages/webui && \
    node /app/node_modules/vite/bin/vite.js build && \
    find /app/packages -type f \
        \( -name "*.d.ts" \
        -o -name "*.d.ts.map" \
        -o -name "*.map" \
        -o -name "*.tsbuildinfo" \) \
        -delete && \
    rm -rf /app/packages/webui/dist/public/fonts

# ================================================================
# Stage 3: Prod Dependencies
# ================================================================

FROM node:24-alpine AS prod-deps
ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME/bin:$PATH"
RUN corepack enable
WORKDIR /app

COPY --from=deemix-builder /app/package.json ./
COPY --from=deemix-builder /app/pnpm-lock.yaml ./
COPY --from=deemix-builder /app/pnpm-workspace.yaml ./
COPY --from=deemix-builder /app/packages/webui/package.json packages/webui/
COPY --from=deemix-builder /app/packages/deemix/package.json packages/deemix/
COPY --from=deemix-builder /app/packages/deezer-sdk/package.json packages/deezer-sdk/

RUN apk add --no-cache python3 make g++ && \
    node -e "const fs=require('fs');const p=JSON.parse(fs.readFileSync('package.json'));delete p.scripts.prepare;fs.writeFileSync('package.json',JSON.stringify(p,null,2));const w=JSON.parse(fs.readFileSync('packages/webui/package.json'));['tailwindcss','postcss','vue','vue-router','pinia','vue-i18n','mitt','toastify-js','lodash-es','flag-icon-css','svg-country-flags'].forEach(pkg=>delete w.dependencies[pkg]);fs.writeFileSync('packages/webui/package.json',JSON.stringify(w,null,2))" && \
    pnpm install --prod

# ================================================================
# Stage 4: Final Image (clean base + only what's needed)
# ================================================================

FROM ghcr.io/linuxserver/baseimage-alpine:3.24

RUN apk add --no-cache nodejs>=24.0.0

# Prod-only node_modules из Stage 2
COPY --from=prod-deps /app/node_modules /app/node_modules
COPY --from=prod-deps /app/packages /app/packages

COPY --from=deemix-builder /app/packages/webui/dist /app/packages/webui/dist
COPY --from=deemix-builder /app/packages/deemix/dist /app/packages/deemix/dist
COPY --from=deemix-builder /app/packages/deezer-sdk/dist /app/packages/deezer-sdk/dist

# S6 сервис deemix
COPY --from=deemix-builder /custom-services.d /custom-services.d

ENV NODE_ENV=production
ENV DEEMIX_DATA_DIR=/config/
ENV DEEMIX_MUSIC_DIR=/downloads/
ENV S6_BEHAVIOUR_IF_STAGE2_FAILS=2

COPY rootfs/ /

COPY --from=telegram_bot-builder /telegram_bot /usr/local/bin/telegram_bot

RUN chmod +x /custom-cont-init.d/* /usr/local/bin/telegram_bot && \
    find /etc/services.d/ -type f -name "run" -exec chmod +x {} \; && \
    find /custom-services.d/ -type f -name "run" -exec chmod +x {} \;

LABEL \
    org.opencontainers.image.title="Home Assistant App: Deemix with telegram bot" \
    org.opencontainers.image.description="High-quality music downloader with a telegram bot: remote, chat-based control for automated downloads." \
    org.opencontainers.image.source="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.licenses="GPL-3.0"
