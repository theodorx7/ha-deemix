#!/usr/bin/with-contenv bash
set -e

echo "[info] Applying Home Assistant options"

if [ ! -f /data/options.json ]; then
    echo "[error] Missing /data/options.json — is this app run by the Supervisor?"
    exit 1
fi

# Publish an environment variable to all S6 services (read via with-contenv)
set_env() {
    printf '%s' "${2}" > "/run/s6/container_environment/${1}"
    export "${1}=${2}"
}

# Custom environment variables from the add-on options (env_vars)
while IFS= read -r -d '' key && IFS= read -r value; do
    set_env "${key}" "${value}"
    echo "[info] env: ${key}=${value}"
done < <(jq -r '.env_vars[]? | "\(.name)\u0000\(.value // "")"' /data/options.json)

# --- Teleemix опции ---
TELEGRAM_ENABLED=$(jq -r '.telegram_enabled // "false"' /data/options.json 2>/dev/null || echo "false")
if [ "${TELEGRAM_ENABLED}" = "true" ]; then
    set_env "TELEGRAM_ENABLED" "${TELEGRAM_ENABLED}"

        # Основные настройки Teleemix
        set_env "TELEGRAM_TOKEN" "$(jq -r '.telegram_token // ""' /data/options.json)"
        set_env "DEEMIX_ARL" "$(jq -r '.deemix_arl // ""' /data/options.json)"
        set_env "DEEMIX_BITRATE" "$(jq -r '.deemix_bitrate // 9' /data/options.json)"
        set_env "DEEMIX_BITRATE_LOCK" "$(jq -r '.deemix_bitrate_lock // false' /data/options.json)"

        # Опциональные API ключи
        set_env "AUDD_API_KEY" "$(jq -r '.audd_api_key // ""' /data/options.json)"
        set_env "OPENAI_API_KEY" "$(jq -r '.openai_api_key // ""' /data/options.json)"
        set_env "WHISPER_URL" "$(jq -r '.whisper_url // ""' /data/options.json)"

        # Внутренние настройки для работы в одном контейнере с deemix
        set_env "DEEMIX_URL" "http://localhost:6595"
        set_env "DEEMIX_SINGLE_USER" "true"
        set_env "USERS_FILE" "/data/teleemix/users.json"
        set_env "RUST_LOG" "info"

    echo "[info] Teleemix Telegram bot enabled"
else
    echo "[info] Teleemix Telegram bot disabled"
fi
# --- КОНЕЦ опций Teleemix ---

# Sync DEEMIX_MUSIC_DIR to config.json (env_vars takes precedence)
if [ -n "${DEEMIX_MUSIC_DIR:-}" ]; then
    CONFIG_FILE="/config/config.json"
    DESIRED="${DEEMIX_MUSIC_DIR%/}/"
    if [ -f "$CONFIG_FILE" ]; then
        CURRENT=$(jq -r '.downloadLocation // empty' "$CONFIG_FILE" 2>/dev/null || echo "")
        if [ "$CURRENT" != "$DESIRED" ]; then
            echo "[info] Syncing downloadLocation to ${DESIRED} (from env_vars)"
            jq --arg loc "$DESIRED" '.downloadLocation = $loc' "$CONFIG_FILE" | cat > "$CONFIG_FILE"
        fi
    fi
fi

echo "[info] Home Assistant options applied"
