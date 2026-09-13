FROM ghcr.io/bambanah/deemix:v4.7.0

ENV S6_BEHAVIOUR_IF_STAGE2_FAILS=2

COPY rootfs/ /

# Docker Overlay: Копируем teleemix binary из готового образа
COPY --from=ghcr.io/theodorx7/ha-teleemix:latest /teleemix /usr/local/bin/teleemix

# Убеждаемся что binary исполняемый + добавляем S6 сервисы
RUN chmod +x /etc/cont-init.d/* /custom-cont-init.d/* /usr/local/bin/teleemix && \
    find /etc/services.d/ -type f -name "run" -exec chmod +x {} \; 2>/dev/null || true

LABEL \
    io.hass.name="Deemix with Telegram Bot" \
    io.hass.description="Deemix - Music Downloader with Telegram Bot" \
    io.hass.type="app" \
    io.hass.url="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.title="Home Assistant App: Deemix" \
    org.opencontainers.image.description="Deemix - Music Downloader with Telegram Bot" \
    org.opencontainers.image.source="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.licenses="GPL-3.0"
    
