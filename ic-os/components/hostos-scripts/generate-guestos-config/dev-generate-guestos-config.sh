#!/bin/bash

set -e

# Generate the GuestOS configuration.

source /opt/ic/bin/logging.sh
source /opt/ic/bin/metrics.sh
source /opt/ic/bin/config.sh

# Get keyword arguments
for argument in "${@}"; do
    case ${argument} in
        -h | --help)
            echo 'Usage:
Generate GuestOS Configuration

Arguments:
  -h, --help            show this help message and exit
  -i=, --input=         specify the input template file (Default: /opt/ic/share/guestos.xml.template)
  -m=, --media=         specify the config media image file (Default: /run/ic-node/config.img)
  -o=, --output=        specify the output configuration file (Default: /var/lib/libvirt/guestos.xml)
'
            exit 1
            ;;
        -i=* | --input=*)
            INPUT="${argument#*=}"
            shift
            ;;
        -m=* | --media=*)
            MEDIA="${argument#*=}"
            shift
            ;;
        -o=* | --output=*)
            OUTPUT="${argument#*=}"
            shift
            ;;
        *)
            echo "Error: Argument is not supported."
            exit 1
            ;;
    esac
done

function validate_arguments() {
    if [ "${INPUT}" == "" -o "${OUTPUT}" == "" ]; then
        $0 --help
    fi
}

# Set arguments if undefined
INPUT="${INPUT:=/opt/ic/share/guestos.xml.template}"
MEDIA="${MEDIA:=/run/ic-node/config.img}"
OUTPUT="${OUTPUT:=/var/lib/libvirt/guestos.xml}"

function read_config_variables() {
    ipv6_prefix=$(get_config_value '.network_settings.ipv6_config.Deterministic.prefix')
    elasticsearch_hosts=$(get_config_value '.icos_settings.logging.elasticsearch_hosts')
    nns_public_key_exists=$(get_config_value '.icos_settings.nns_public_key_exists')
    nns_urls=$(get_config_value '.icos_settings.nns_urls | join(",")')
    node_operator_private_key_exists=$(get_config_value '.icos_settings.node_operator_private_key_exists')
    vm_memory=$(get_config_value '.hostos_settings.vm_memory')
    vm_cpu=$(get_config_value '.hostos_settings.vm_cpu')
    use_ssh_authorized_keys=$(get_config_value '.icos_settings.use_ssh_authorized_keys')
}

function assemble_config_media() {
    ipv6_address="$(/opt/ic/bin/hostos_tool generate-ipv6-address --node-type GuestOS)"
    /opt/ic/bin/config generate-guestos-config --guestos-ipv6-address "$ipv6_address"

    cmd=(/opt/ic/bin/build-bootstrap-config-image.sh ${MEDIA})
    cmd+=(--guestos_config "/boot/config/config-guestos.json")
    if [[ "${nns_public_key_exists,,}" == "true" ]]; then
        cmd+=(--nns_public_key "/boot/config/nns_public_key.pem")
    fi
    if [ -f "$node_operator_private_key_exists" ]; then
        cmd+=(--node_operator_private_key "/boot/config/node_operator_private_key.pem")
    fi
    if [[ "${ssh_authorized_keys,,}" == "true" ]]; then
        cmd+=(--accounts_ssh_authorized_keys "/boot/config/ssh_authorized_keys")
    fi

    # Run the above command
    "${cmd[@]}"
    write_log "Assembling config media for GuestOS: ${MEDIA}"
}

function generate_guestos_config() {
    MAC_ADDRESS=$(/opt/ic/bin/hostos_tool generate-mac-address --node-type GuestOS)

    CPU_DOMAIN="kvm"
    CPU_SPEC="/opt/ic/share/kvm-cpu.xml"
    if [ "${vm_cpu}" == "qemu" ]; then
        CPU_DOMAIN="qemu"
        CPU_SPEC="/opt/ic/share/qemu-cpu.xml"
    fi

    if [ ! -f "${OUTPUT}" ]; then
        mkdir -p "$(dirname "$OUTPUT")"
        sed -e "s@{{ resources_memory }}@${vm_memory}@" \
            -e "s@{{ mac_address }}@${MAC_ADDRESS}@" \
            -e "s@{{ cpu_domain }}@${CPU_DOMAIN}@" \
            -e "/{{ cpu_spec }}/{r ${CPU_SPEC}" -e "d" -e "}" \
            "${INPUT}" >"${OUTPUT}"
        restorecon -R "$(dirname "$OUTPUT")"
        write_log "Generating GuestOS configuration file: ${OUTPUT}"
        write_metric "hostos_generate_guestos_config" \
            "1" \
            "HostOS generate GuestOS config" \
            "gauge"
    else
        write_log "GuestOS configuration file already exists: ${OUTPUT}"
        write_metric "hostos_generate_guestos_config" \
            "0" \
            "HostOS generate GuestOS config" \
            "gauge"
    fi
}

function main() {
    validate_arguments
    read_config_variables
    assemble_config_media
    generate_guestos_config
}

main
