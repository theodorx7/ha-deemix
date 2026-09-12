# ================================================================
# Сборка deemix из исходников на HA base image
# ================================================================

# Base: HA image (содержит S6, bashio, tempio)
FROM ghcr.io/home-assistant/base:latest

ARG BUILD_VERSION

# Устанавливаем Node.js и инструменты для сборки
RUN apk add --no-cache \
    nodejs \
    git \
    python3 \
    make \
    g++

# Включаем pnpm и turbo
RUN corepack enable
RUN pnpm add -g turbo

# Клонируем исходники deemix
RUN git clone --depth 1 \
    https://github.com/bambanah/deemix.git \
    /app

WORKDIR /app

# Turbo prune — выделяем только webui и cli
RUN turbo prune deemix-webui deemix-cli --docker

# Переходим в pruned директорию
WORKDIR /app/out

# Устанавливаем зависимости
RUN pnpm install --frozen-lockfile

# Собираем webui и cli
RUN pnpm turbo build --filter=deemix-webui... --filter=deemix-cli...

# Удаляем исходники и build tools для уменьшения размера
RUN apk del git python3 make g++ && \
    rm -rf /var/cache/apk/*

# ================================================================
# Копируем наши HA-скрипты
# ================================================================
COPY rootfs/ /

RUN find /etc/cont-init.d/ -type f -name "*.sh" -exec chmod +x {} \; && \
    find /etc/services.d/ -type f -name "run" -exec chmod +x {} \;

# ================================================================
# HA Labels
# ================================================================
LABEL \
    io.hass.type="app" \
    io.hass.arch="aarch64|amd64" \
    io.hass.version=${BUILD_VERSION} \
    io.hass.name="Deemix" \
    io.hass.description="Deemix - Music Downloader" \
    io.hass.url="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.title="Home Assistant App: Deemix" \
    org.opencontainers.image.description="Deemix - Music Downloader" \
    org.opencontainers.image.version=${BUILD_VERSION} \
    org.opencontainers.image.source="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.licenses="GPL-3.0"

EXPOSE 6595

# HA S6 entrypoint
ENTRYPOINT [ "/init" ]
