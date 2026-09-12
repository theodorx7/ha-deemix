FROM ghcr.io/home-assistant/base:latest

# Устанавливаем Node.js для deemix
RUN apk add --no-cache nodejs npm

# Копируем rootfs с нашими скриптами
COPY rootfs/ /

# Делаем скрипты исполняемыми
RUN find /etc/cont-init.d/ -type f -name "*.sh" -exec chmod +x {} \; && \
    find /etc/services.d/ -type f -name "run" -exec chmod +x {} \;

LABEL \
    io.hass.type="app" \
    io.hass.arch="aarch64|amd64" \
    io.hass.version="4.0.7-1" \
    org.opencontainers.image.title="Home Assistant App: Deemix" \
    org.opencontainers.image.description="Deemix - Music Downloader" \
    org.opencontainers.image.source="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.documentation="https://github.com/theodorx7/ha-deemix/blob/main/README.md"
    org.opencontainers.image.licenses="GPL-3.0"

# Port для веб-интерфейса
EXPOSE 6595

# S6 entrypoint (уже в base image)
ENTRYPOINT [ "/init" ]
