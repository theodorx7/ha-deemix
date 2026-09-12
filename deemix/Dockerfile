# Используем HA base image (включает S6, bashio, tempio)
ARG BUILD_FROM=ghcr.io/home-assistant/base:latest
FROM ${BUILD_FROM}

# Устанавливаем Node.js для deemix
RUN apk add --no-cache nodejs npm

# Копируем rootfs с нашими скриптами
COPY rootfs/ /

# Делаем скрипты исполняемыми
RUN find /etc/cont-init.d/ -type f -name "*.sh" -exec chmod +x {} \; && \
    find /etc/services.d/ -type f -name "run" -exec chmod +x {} \;

# HA Labels
LABEL \
    io.hass.type="app" \
    io.hass.arch="aarch64|amd64" \
    io.hass.version="1.0.0" \
    org.opencontainers.image.title="Home Assistant App: Deemix" \
    org.opencontainers.image.description="Deemix - Music Downloader" \
    org.opencontainers.image.source="https://github.com/ваш-username/ha-deemix" \
    org.opencontainers.image.licenses="MIT"

# Port для веб-интерфейса
EXPOSE 6595

# S6 entrypoint (уже в base image)
ENTRYPOINT [ "/init" ]
