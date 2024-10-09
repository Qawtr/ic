#!/bin/bash

# Shared config utilities.

# Retrieves a value from the config.json file using a JSON path.
# Arguments:
#   $1 - JSON path to the desired value (e.g., '.icos_settings.node_operator_private_key_path')
function get_config_value() {
    local CONFIG_FILE="/var/ic/config/config.json"
    local key=$1
    jq -r "${key}" "${CONFIG_FILE}"
}
