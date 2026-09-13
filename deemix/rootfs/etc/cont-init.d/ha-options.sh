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

# Sync DEEMIX_MUSIC_DIR to config.json (env_vars takes precedence)
if [ -n "${DEEMIX_MUSIC_DIR:-}" ]; then
    CONFIG_FILE="/config/config.json"
    DESIRED="${DEEMIX_MUSIC_DIR%/}/"
    if [ -f "$CONFIG_FILE" ]; then
        CURRENT=$(jq -r '.downloadLocation // empty' "$CONFIG_FILE" 2>/dev/null || echo "")
        if [ "$CURRENT" != "$DESIRED" ]; then
            echo "[info] Syncing downloadLocation to ${DESIRED} (from env_vars)"
            jq --arg loc "$DESIRED" '.downloadLocation = $loc' "$CONFIG_FILE" > /tmp/config.tmp && \
                mv /tmp/config.tmp "$CONFIG_FILE"
        fi
    fi
fi

echo "[info] Home Assistant options applied"
