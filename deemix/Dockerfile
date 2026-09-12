ARG BUILD_FROM=ghcr.io/home-assistant/base:latest
FROM ${BUILD_FROM}

# Устанавливаем Node.js (deemix - это Node.js приложение)
RUN apk add --no-cache nodejs npm

# Устанавливаем bashio (если еще не в base image)
# (обычно уже есть в HA base image)

# Копируем кастомные скрипты
COPY rootfs/ /

# Добавляем HA labels
LABEL \
    io.hass.type="app" \
    io.hass.arch="aarch64|amd64"

# Используем HA S6
ENTRYPOINT [ "/init" ]
