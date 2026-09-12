
#!/usr/bin/env bashio
# shellcheck shell=bash
set -e

bashio::log.info "Starting Deemix configuration..."

# Создание необходимых директорий
bashio::log.info "Creating directories..."
mkdir -p /config
mkdir -p /media/deemix
chown -R 1000:1000 /config /media/deemix

# Настройка переменных окружения из опций HA
export DEEMIX_SERVER_PORT="$(bashio::config 'server_port')"
export DEEMIX_HOST="$(bashio::config 'host')"

bashio::log.info "Server Port: ${DEEMIX_SERVER_PORT}"
bashio::log.info "Host: ${DEEMIX_HOST}"

# SSL конфигурация (по аналогии с navidrome)
if bashio::config.true 'ssl'; then
    bashio::log.info "SSL is enabled"
    certfile="$(bashio::config 'certfile')"
    keyfile="$(bashio::config 'keyfile')"
    
    # Проверка SSL сертификатов
    if [ -f "/ssl/${certfile}" ] && [ -f "/ssl/${keyfile}" ]; then
        bashio::log.info "SSL certificates found"
    else
        bashio::log.warning "SSL certificates not found in /ssl/"
    fi
fi

# Исправление config.json (по аналогии с оригинальным deemix docker)
if [ -f "/config/config.json" ]; then
    bashio::log.info "Fixing config.json..."
    # Убеждаемся, что download location правильный
    jq '.downloadLocation = "/media/deemix"' /config/config.json > /tmp/config.tmp && \
        mv /tmp/config.tmp /config/config.json
fi

bashio::log.info "Deemix configuration completed"
