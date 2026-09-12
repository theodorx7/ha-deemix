

# ================================================================
# Копируем наши HA-скрипты
# ================================================================
COPY rootfs/ /

RUN find /etc/cont-init.d/ -type f -name "*.sh" -exec chmod +x {} \; && \
    find /etc/services.d/ -type f -name "run" -exec chmod +x {} \;

LABEL \
    io.hass.name="Deemix" \
    io.hass.description="Deemix - Music Downloader" \
    io.hass.version=${BUILD_VERSION} \
    io.hass.type="app" \
    io.hass.arch="aarch64|amd64" \
    io.hass.url="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.title="Deemix" \
    org.opencontainers.image.description="Deemix - Music Downloader" \
    org.opencontainers.image.source="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.licenses="GPL-3.0"

EXPOSE 6595

# HA S6 entrypoint
ENTRYPOINT [ "/init" ]
