FROM ghcr.io/bambanah/deemix:v4.7.0

ENV S6_BEHAVIOUR_IF_STAGE2_FAILS=2

COPY rootfs/ /

# Docker Overlay: Копируем teleemix binary из готового образа
COPY --from=ghcr.io/theodorx7/ha-teleemix:latest /teleemix /usr/local/bin/teleemix

RUN find /etc/cont-init.d/ -type f -exec chmod +x {} \; && \
    find /etc/services.d/ -type f -name "run" -exec chmod +x {} \; && chmod +x /usr/local/bin/teleemix

LABEL \
    io.hass.name="Deemix with Telegram Bot" \
    io.hass.description="High-quality music downloader with a Telegram bot: Remote, chat-based control for automated downloads." \
    io.hass.type="app" \
    io.hass.url="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.title="Home Assistant App: Deemix with Telegram Bot" \
    org.opencontainers.image.description="High-quality music downloader with a Telegram bot: Remote, chat-based control for automated downloads." \
    org.opencontainers.image.source="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.licenses="GPL-3.0"
    
