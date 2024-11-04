"""
Enumerate every component file dependency for GuestOS
"""

component_files = {
    # early-boot
    Label("early-boot/relabel-machine-id/guestos/relabel-machine-id.sh"): "/opt/ic/bin/relabel-machine-id.sh",
    Label("early-boot/relabel-machine-id/relabel-machine-id.service"): "/etc/systemd/system/relabel-machine-id.service",
    Label("early-boot/setup-hostname/setup-hostname.sh"): "/opt/ic/bin/setup-hostname.sh",
    Label("early-boot/setup-hostname/setup-hostname.service"): "/etc/systemd/system/setup-hostname.service",
    Label("early-boot/setup-hostname/hostname-empty"): "/etc/hostname",
    Label("early-boot/save-machine-id/save-machine-id.sh"): "/opt/ic/bin/save-machine-id.sh",
    Label("early-boot/save-machine-id/save-machine-id.service"): "/etc/systemd/system/save-machine-id.service",
    Label("early-boot/fstab/fstab-guestos"): "/etc/fstab",
    Label("early-boot/locale"): "/etc/default/locale",
    Label("early-boot/initramfs-tools/guestos/hooks/veritysetup"): "/etc/initramfs-tools/hooks/veritysetup",
    Label("early-boot/initramfs-tools/guestos/initramfs.conf"): "/etc/initramfs-tools/initramfs.conf",
    Label("early-boot/initramfs-tools/guestos/modules"): "/etc/initramfs-tools/modules",
    Label("early-boot/initramfs-tools/guestos/scripts/init-bottom/set-machine-id"): "/etc/initramfs-tools/scripts/init-bottom/set-machine-id",
    Label("early-boot/initramfs-tools/guestos/scripts/init-premount/verity-root"): "/etc/initramfs-tools/scripts/init-premount/verity-root",

    # ic
    Label("ic/ic-btc-adapter/generate-btc-adapter-config.sh"): "/opt/ic/bin/generate-btc-adapter-config.sh",
    Label("ic/ic-btc-adapter/ic-btc-mainnet-adapter.service"): "/etc/systemd/system/ic-btc-mainnet-adapter.service",
    Label("ic/ic-btc-adapter/ic-btc-mainnet-adapter.socket"): "/etc/systemd/system/ic-btc-mainnet-adapter.socket",
    Label("ic/ic-btc-adapter/ic-btc-testnet-adapter.service"): "/etc/systemd/system/ic-btc-testnet-adapter.service",
    Label("ic/ic-btc-adapter/ic-btc-testnet-adapter.socket"): "/etc/systemd/system/ic-btc-testnet-adapter.socket",
    Label("ic/setup-permissions/setup-permissions.sh"): "/opt/ic/bin/setup-permissions.sh",
    Label("ic/setup-permissions/setup-permissions.service"): "/etc/systemd/system/setup-permissions.service",
    Label("ic/share/ark.pem"): "/opt/ic/share/ark.pem",
    Label("ic/ic-crypto-csp/ic-crypto-csp.service"): "/etc/systemd/system/ic-crypto-csp.service",
    Label("ic/ic-crypto-csp/ic-crypto-csp.socket"): "/etc/systemd/system/ic-crypto-csp.socket",
    Label("ic/ic-https-outcalls-adapter/ic-https-outcalls-adapter.service"): "/etc/systemd/system/ic-https-outcalls-adapter.service",
    Label("ic/ic-https-outcalls-adapter/ic-https-outcalls-adapter.socket"): "/etc/systemd/system/ic-https-outcalls-adapter.socket",
    Label("ic/ic-https-outcalls-adapter/generate-https-outcalls-adapter-config.sh"): "/opt/ic/bin/generate-https-outcalls-adapter-config.sh",
    Label("ic/ic-replica.service"): "/etc/systemd/system/ic-replica.service",
    Label("ic/generate-replica-config.sh"): "/opt/ic/bin/generate-replica-config.sh",
    Label("ic/ic.json5.template"): "/opt/ic/share/ic.json5.template",

    # init
    Label("init/bootstrap-ic-node/bootstrap-ic-node.sh"): "/opt/ic/bin/bootstrap-ic-node.sh",
    Label("init/bootstrap-ic-node/bootstrap-ic-node.service"): "/etc/systemd/system/bootstrap-ic-node.service",
    Label("init/setup-encryption/setup-encryption.sh"): "/opt/ic/bin/setup-encryption.sh",
    Label("init/setup-encryption/setup-encryption.service"): "/etc/systemd/system/setup-encryption.service",
    Label("init/setup-encryption/setup-var-encryption.sh"): "/opt/ic/bin/setup-var-encryption.sh",
    Label("init/setup-lvs/setup-lvs.service"): "/etc/systemd/system/setup-lvs.service",
    Label("init/setup-lvs/guestos/setup-lvs.sh"): "/opt/ic/bin/setup-lvs.sh",

    # misc
    Label("misc/config/config.sh"): "/opt/ic/bin/config.sh",
    Label("misc/logging.sh"): "/opt/ic/bin/logging.sh",
    Label("misc/metrics.sh"): "/opt/ic/bin/metrics.sh",
    Label("misc/serial-getty@/guestos/override.conf"): "/etc/systemd/system/serial-getty@.service.d/override.conf",
    Label("misc/chrony/chrony.conf"): "/etc/chrony/chrony.conf",
    Label("misc/chrony/chrony-var.service"): "/etc/systemd/system/chrony-var.service",
    Label("misc/vsock/10-vhost-vsock.rules"): "/etc/udev/rules.d/10-vhost-vsock.rules",
    Label("misc/guestos/ic-node.conf"): "/etc/tmpfiles.d/ic-node.conf",
    Label("misc/guestos/sudoers"): "/etc/sudoers",
    Label("misc/guestos/crypttab"): "/etc/crypttab",
    Label("misc/guestos/sysctl.d/dfn-max-map-count.conf"): "/etc/sysctl.d/dfn-max-map-count.conf",
    Label("misc/guestos/sysctl.d/privileged-ports.conf"): "/etc/sysctl.d/privileged-ports.conf",
    Label("misc/guestos/sysfs.d/hugepage.conf"): "/etc/sysfs.d/hugepage.conf",
    Label("misc/log-config/log-config.service"): "/etc/systemd/system/log-config.service",
    Label("misc/log-config/log-config.sh"): "/opt/ic/bin/log-config.sh",

    # monitoring
    Label("monitoring/filebeat/setup-filebeat-permissions.sh"): "/opt/ic/bin/setup-filebeat-permissions.sh",
    Label("monitoring/filebeat/generate-filebeat-config.sh"): "/opt/ic/bin/generate-filebeat-config.sh",
    Label("monitoring/filebeat/filebeat.yml.template"): "/etc/filebeat/filebeat.yml.template",
    Label("monitoring/filebeat/filebeat.service"): "/etc/systemd/system/filebeat.service",
    Label("monitoring/ipv4-connectivity-check/ipv4-connectivity-check.sh"): "/opt/ic/bin/ipv4-connectivity-check.sh",
    Label("monitoring/ipv4-connectivity-check/ipv4-connectivity-check.service"): "/etc/systemd/system/ipv4-connectivity-check.service",
    Label("monitoring/ipv4-connectivity-check/ipv4-connectivity-check.timer"): "/etc/systemd/system/ipv4-connectivity-check.timer",
    Label("monitoring/systemd-user/user@.service"): "/etc/systemd/system/user@.service",
    Label("monitoring/fstrim/fstrim_tool.service"): "/etc/systemd/system/fstrim_tool.service",
    Label("monitoring/fstrim/fstrim_tool.timer"): "/etc/systemd/system/fstrim_tool.timer",
    Label("monitoring/fstrim/setup-fstrim-metrics.service"): "/etc/systemd/system/setup-fstrim-metrics.service",
    Label("monitoring/setup-node-gen-status.service"): "/etc/systemd/system/setup-node-gen-status.service",
    Label("monitoring/node_exporter/node_exporter.crt"): "/etc/node_exporter/node_exporter.crt",
    Label("monitoring/node_exporter/node_exporter.key"): "/etc/node_exporter/node_exporter.key",
    Label("monitoring/node_exporter/web.yml"): "/etc/node_exporter/web.yml",
    Label("monitoring/node_exporter/node_exporter.service"): "/etc/systemd/system/node_exporter.service",
    Label("monitoring/node_exporter/node_exporter"): "/etc/default/node_exporter",
    Label("monitoring/node_exporter/setup-node_exporter-keys/setup-node_exporter-keys.sh"): "/opt/ic/bin/setup-node_exporter-keys.sh",
    Label("monitoring/node_exporter/setup-node_exporter-keys/setup-node_exporter-keys.service"): "/etc/systemd/system/setup-node_exporter-keys.service",
    Label("monitoring/metrics-proxy/guestos/metrics-proxy.yaml"): "/etc/metrics-proxy.yaml",
    Label("monitoring/metrics-proxy/metrics-proxy.service"): "/etc/systemd/system/metrics-proxy.service",
    Label("monitoring/journald.conf"): "/etc/systemd/journald.conf",
    Label("monitoring/nft-exporter/nft-exporter.service"): "/etc/systemd/system/nft-exporter.service",
    Label("monitoring/nft-exporter/nft-exporter.timer"): "/etc/systemd/system/nft-exporter.timer",
    Label("monitoring/custom-metrics/metrics_tool.service"): "/etc/systemd/system/metrics_tool.service",
    Label("monitoring/custom-metrics/metrics_tool.timer"): "/etc/systemd/system/metrics_tool.timer",

    # networking
    Label("networking/generate-network-config/guestos/generate-network-config.service"): "/etc/systemd/system/generate-network-config.service",
    Label("networking/retry-ipv6-config/retry-ipv6-config.sh"): "/opt/ic/bin/retry-ipv6-config.sh",
    Label("networking/retry-ipv6-config/retry-ipv6-config.service"): "/etc/systemd/system/retry-ipv6-config.service",
    Label("networking/nftables/reload_nftables.path"): "/etc/systemd/system/reload_nftables.path",
    Label("networking/nftables/reload_nftables.service"): "/etc/systemd/system/reload_nftables.service",
    Label("networking/nftables/nftables-empty.conf"): "/etc/nftables.conf",
    Label("networking/fallback.conf"): "/etc/systemd/resolved.conf.d/fallback.conf",
    Label("networking/resolv.conf"): "/etc/resolv.conf",
    Label("networking/network-tweaks.conf"): "/etc/sysctl.d/network-tweaks.conf",
    Label("networking/hosts"): "/etc/hosts",
    Label("networking/dev-certs/canister_http_test_ca.cert"): "/dev-certs/canister_http_test_ca.cert",

    # SELinux
    Label("selinux/filebeat/filebeat.fc"): "/prep/filebeat/filebeat.fc",
    Label("selinux/filebeat/filebeat.if"): "/prep/filebeat/filebeat.if",
    Label("selinux/filebeat/filebeat.te"): "/prep/filebeat/filebeat.te",
    Label("selinux/manageboot/manageboot.fc"): "/prep/manageboot/manageboot.fc",
    Label("selinux/manageboot/manageboot.if"): "/prep/manageboot/manageboot.if",
    Label("selinux/manageboot/manageboot.te"): "/prep/manageboot/manageboot.te",
    Label("selinux/fscontext-fixes/fscontext-fixes.fc"): "/prep/fscontext-fixes/fscontext-fixes.fc",
    Label("selinux/fscontext-fixes/fscontext-fixes.if"): "/prep/fscontext-fixes/fscontext-fixes.if",
    Label("selinux/fscontext-fixes/fscontext-fixes.te"): "/prep/fscontext-fixes/fscontext-fixes.te",
    Label("selinux/ic-node/ic-node.fc"): "/prep/ic-node/ic-node.fc",
    Label("selinux/ic-node/ic-node.if"): "/prep/ic-node/ic-node.if",
    Label("selinux/ic-node/ic-node.te"): "/prep/ic-node/ic-node.te",
    Label("selinux/infogetty/infogetty.fc"): "/prep/infogetty/infogetty.fc",
    Label("selinux/infogetty/infogetty.te"): "/prep/infogetty/infogetty.te",
    Label("selinux/misc-fixes/misc-fixes.if"): "/prep/misc-fixes/misc-fixes.if",
    Label("selinux/misc-fixes/misc-fixes.te"): "/prep/misc-fixes/misc-fixes.te",
    Label("selinux/node_exporter/node_exporter.fc"): "/prep/node_exporter/node_exporter.fc",
    Label("selinux/node_exporter/node_exporter.if"): "/prep/node_exporter/node_exporter.if",
    Label("selinux/node_exporter/node_exporter.te"): "/prep/node_exporter/node_exporter.te",
    Label("selinux/prep.sh"): "/prep/prep.sh",
    Label("selinux/setup-var/setup-var.if"): "/prep/setup-var/setup-var.if",
    Label("selinux/setup-var/setup-var.te"): "/prep/setup-var/setup-var.te",
    Label("selinux/systemd-fixes/systemd-fixes.if"): "/prep/systemd-fixes/systemd-fixes.if",
    Label("selinux/systemd-fixes/systemd-fixes.te"): "/prep/systemd-fixes/systemd-fixes.te",

    # ssh
    Label("ssh/provision-ssh-keys.sh"): "/opt/ic/bin/provision-ssh-keys.sh",
    Label("ssh/setup-ssh-keys/setup-ssh-keys.sh"): "/opt/ic/bin/setup-ssh-keys.sh",
    Label("ssh/setup-ssh-keys/setup-ssh-keys.service"): "/etc/systemd/system/setup-ssh-keys.service",
    Label("ssh/setup-ssh-account-keys/setup-ssh-account-keys.sh"): "/opt/ic/bin/setup-ssh-account-keys.sh",
    Label("ssh/setup-ssh-account-keys/setup-ssh-account-keys.service"): "/etc/systemd/system/setup-ssh-account-keys.service",
    Label("ssh/read-ssh-keys.sh"): "/opt/ic/bin/read-ssh-keys.sh",

    # upgrade
    Label("upgrade/shared-resources/monitor-expand-shared-data/monitor-expand-shared-data.service"): "/etc/systemd/system/monitor-expand-shared-data.service",
    Label("upgrade/shared-resources/upgrade-shared-data-store/upgrade-shared-data-store.sh"): "/opt/ic/bin/upgrade-shared-data-store.sh",
    Label("upgrade/shared-resources/upgrade-shared-data-store/upgrade-shared-data-store.service"): "/etc/systemd/system/upgrade-shared-data-store.service",
    Label("upgrade/shared-resources/setup-shared-resources/setup-shared-backup.sh"): "/opt/ic/bin/setup-shared-backup.sh",
    Label("upgrade/shared-resources/setup-shared-resources/setup-shared-backup.service"): "/etc/systemd/system/setup-shared-backup.service",
    Label("upgrade/shared-resources/setup-shared-resources/setup-shared-crypto.sh"): "/opt/ic/bin/setup-shared-crypto.sh",
    Label("upgrade/shared-resources/setup-shared-resources/setup-shared-crypto.service"): "/etc/systemd/system/setup-shared-crypto.service",
    Label("upgrade/shared-resources/setup-shared-resources/setup-shared-data.sh"): "/opt/ic/bin/setup-shared-data.sh",
    Label("upgrade/shared-resources/setup-shared-resources/setup-shared-data.service"): "/etc/systemd/system/setup-shared-data.service",
    Label("upgrade/systemd-generators/guestos/mount-generator"): "/etc/systemd/system-generators/mount-generator",
    Label("upgrade/systemd-generators/systemd-gpt-auto-generator"): "/etc/systemd/system-generators/systemd-gpt-auto-generator",
    Label("upgrade/manageboot/manageboot.sh"): "/opt/ic/bin/manageboot.sh",
    Label("upgrade/shared-resources/monitor-expand-shared-data/monitor-expand-shared-data.py"): "/opt/ic/bin/monitor-expand-shared-data.py",
}
