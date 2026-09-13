FROM ghcr.io/bambanah/deemix:v4.7.0

ENV S6_BEHAVIOUR_IF_STAGE2_FAILS=2

COPY rootfs/ /
RUN chmod +x /etc/cont-init.d/* /custom-cont-init.d/*

LABEL \
    io.hass.name="Deemix" \
    io.hass.description="Deemix - Music Downloader" \
    io.hass.type="app" \
    io.hass.url="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.title="Home Assistant App: Deemix" \
    org.opencontainers.image.description="Deemix - Music Downloader" \
    org.opencontainers.image.source="https://github.com/theodorx7/ha-deemix" \
    org.opencontainers.image.licenses="GPL-3.0"
    
