FROM ghcr.io/bambanah/deemix:v4.7.0

# bashio — official Home Assistant app script library (not present in LSIO base)
ARG BASHIO_VERSION=0.17.5
RUN \
    curl -L -s -o /tmp/bashio.tar.gz \
        "https://github.com/hassio-addons/bashio/archive/v${BASHIO_VERSION}.tar.gz" \
    && mkdir -p /tmp/bashio \
    && tar zxf /tmp/bashio.tar.gz --strip 1 -C /tmp/bashio \
    && mv /tmp/bashio/lib /usr/lib/bashio \
    && ln -s /usr/lib/bashio/bashio /usr/bin/bashio \
    && rm -rf /tmp/bashio /tmp/bashio.tar.gz

# Defaults for LSIO user mapping (overridable via env_vars); without them
# the upstream chown calls in 10-fix_folders fail silently
ENV PUID=911 PGID=911

# Fail fast on init errors (same behaviour as HA base images)
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

