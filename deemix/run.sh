#!/usr/bin/with-contenv bashio

set -e
bashio::log.info "Starting Deemix App..."
export DEEMIX_SERVER_PORT="$(bashio::config 'server_port')"
export DEEMIX_HOST="$(bashio::config 'host')"
bashio::log.info "Server Port: ${DEEMIX_SERVER_PORT}, Host: ${DEEMIX_HOST}"
exec /init
