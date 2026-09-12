FROM ghcr.io/bambanah/deemix:v4.7.0

# Defaults for LSIO user mapping (overridable via env_vars)
ENV PUID=911 PGID=911
# Override upstream default (/downloads/) — /media is where HA maps it
ENV DEEMIX_MUSIC_DIR=/media/Downloads/

# Fail fast on init errors
ENV S6_BEHAVIOUR_IF_STAGE2_FAILS=2

# Home Assistant specific root filesystem additions
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

