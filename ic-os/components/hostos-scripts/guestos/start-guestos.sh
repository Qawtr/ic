#!/bin/bash

set -e

# Start the GuestOS virtual machine.

source /opt/ic/bin/logging.sh
source /opt/ic/bin/metrics.sh

SCRIPT="$(basename $0)[$$]"

# Get keyword arguments
for argument in "${@}"; do
    case ${argument} in
        -h | --help)
            echo 'Usage:
Start GuestOS virtual machine

Arguments:
  -c=, --config=        specify the GuestOS configuration file (Default: /var/lib/libvirt/guestos.xml)
  -h, --help            show this help message and exit
'
            exit 1
            ;;
        *)
            echo "Error: Argument is not supported."
            exit 1
            ;;
    esac
done

# Set arguments if undefined
CONFIG="${CONFIG:=/var/lib/libvirt/guestos.xml}"

function setup_sev_mounts() {
    if [ "$(mount | grep 'sev-boot-components')" ]; then
        write_log "SEV boot components are already ready."
    else
        write_log "Setting up SEV boot components."
        mkdir -p /tmp/sev-boot-components/
        losetup -P /dev/loop99 /dev/mapper/hostlvm-guestos
        mount /dev/loop99p4 /tmp/sev-boot-components/
    fi
}

function define_guestos() {
    if [ "$(virsh list --all | grep 'guestos')" ]; then
        write_log "GuestOS virtual machine is already defined."
        write_metric "hostos_guestos_service_define" \
            "0" \
            "GuestOS virtual machine define state" \
            "gauge"
    else
        virsh define ${CONFIG}
        write_log "Defining GuestOS virtual machine."
        write_metric "hostos_guestos_service_define" \
            "1" \
            "GuestOS virtual machine define state" \
            "gauge"
    fi
}

function start_guestos() {
    if [ "$(virsh list --state-running | grep 'guestos')" ]; then
        write_log "GuestOS virtual machine is already running."
        write_metric "hostos_guestos_service_start" \
            "0" \
            "GuestOS virtual machine start state" \
            "gauge"
    else
        virsh start guestos
        write_log "Starting GuestOS virtual machine."
        write_metric "hostos_guestos_service_start" \
            "1" \
            "GuestOS virtual machine start state" \
            "gauge"
    fi
}

function main() {
    # Establish run order
    setup_sev_mounts
    define_guestos
    start_guestos
}

main
