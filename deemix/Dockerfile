# ================================================================
# ОДИН Dockerfile — собираем deemix из исходников на HA base
# ================================================================

# Base: HA image (содержит S6, bashio, tempio)
FROM ghcr.io/home-assistant/base:latest

# Устанавливаем Node.js и инструменты для сборки
RUN apk add --no-cache \
    nodejs \
    npm \
    git \
    python3 \
    make \
    g++

# Включаем pnpm (менеджер пакетов для deemix)
RUN corepack enable

# Клонируем исходники deemix
RUN git clone --depth 1 \
    https://github.com/bambanah/deemix.git \
    /app

WORKDIR /app

# Устанавливаем зависимости
RUN pnpm install --frozen-lockfile

# Собираем webui (основной компонент)
RUN pnpm turbo build --filter=deemix-webui...

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
    io.hass.version="4.0.7-1" \
    io.hass.name="Deemix" \
    io.hass.description="Deemix - Music Downloader" \
    io.hass.url="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.title="Home Assistant App: Deemix" \
    org.opencontainers.image.description="Deemix - Music Downloader" \
    org.opencontainers.image.version="4.0.7-1" \
    org.opencontainers.image.source="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.licenses="GPL-3.0"

# ================================================================
# Конфигурация
# ================================================================
ENV DEEMIX_DATA_DIR="/config" \
    DEEMIX_MUSIC_DIR="/media" \
    DEEMIX_SERVER_PORT="6595" \
    DEEMIX_HOST="0.0.0.0" \
    DEEMIX_SINGLE_USER="true" \
    NODE_ENV="production"

EXPOSE 6595

# HA S6 entrypoint
ENTRYPOINT [ "/init" ]
