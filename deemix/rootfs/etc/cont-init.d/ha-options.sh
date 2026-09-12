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

echo "[info] Home Assistant options applied"
