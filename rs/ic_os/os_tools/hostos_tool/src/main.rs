use std::path::Path;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};

use config::config_ini::config_map_from_path;
use config::deployment_json::get_deployment_settings;
use config::firewall_json;
use config::types::firewall;
use config::{
    DEFAULT_HOSTOS_CONFIG_INI_FILE_PATH, DEFAULT_HOSTOS_DEPLOYMENT_JSON_PATH,
    DEFAULT_HOSTOS_FIREWALL_JSON_PATH,
};
use mac_address::mac_address::{generate_mac_address, get_ipmi_mac, FormattedMacAddress};
use mac_address::node_type::NodeType;
use network::generate_network_config;
use network::info::NetworkInfo;
use network::ipv6::generate_ipv6_address;
use network::systemd::DEFAULT_SYSTEMD_NETWORK_DIR;
use utils::to_cidr;

#[derive(Subcommand)]
pub enum Commands {
    /// Generate systemd network configuration files. Bridges available NIC's for IC IPv6 connectivity.
    GenerateNetworkConfig {
        #[arg(short, long, default_value_t = DEFAULT_SYSTEMD_NETWORK_DIR.to_string(), value_name = "DIR")]
        /// systemd-networkd output directory
        output_directory: String,
    },
    GenerateIpv6Address {
        #[arg(short, long, default_value = "HostOS")]
        node_type: String,
    },
    GenerateMacAddress {
        #[arg(short, long, default_value = "HostOS")]
        node_type: String,
    },
    FetchMacAddress {},
    RenderFirewallConfig {
        #[arg(index = 1)]
        /// Path to firewall.json.  Defaults to DEFAULT_HOSTOS_FIREWALL_JSON_PATH if unspecified.
        /// If the option is not specified, and the default file does not exist, it renders an
        /// empty firewall ruleset.  If the option is specified, and the file does not exist,
        /// it will raise an error.  If the file exists but the rules cannot be read, it will
        /// raise an error.
        firewall_file: Option<String>,
    },
}

#[derive(Parser)]
struct HostOSArgs {
    #[arg(short, long, default_value_t = DEFAULT_HOSTOS_CONFIG_INI_FILE_PATH.to_string(), value_name = "FILE")]
    config: String,

    #[arg(short, long, default_value_t = DEFAULT_HOSTOS_DEPLOYMENT_JSON_PATH.to_string(), value_name = "FILE")]
    /// deployment.json file path
    deployment_file: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

pub fn main() -> Result<()> {
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("ERROR: this only runs on Linux.");
        std::process::exit(1);
    }

    let opts = HostOSArgs::parse();

    match opts.command {
        Some(Commands::GenerateNetworkConfig { output_directory }) => {
            let config_map = config_map_from_path(Path::new(&opts.config)).context(format!(
                "Failed to get config.ini settings for path: {}",
                &opts.config
            ))?;
            eprintln!("Using config: {:?}", config_map);

            let network_info = NetworkInfo::from_config_map(&config_map)?;
            eprintln!("Network info config: {:?}", &network_info);

            let deployment_settings = get_deployment_settings(Path::new(&opts.deployment_file))
                .context(format!(
                    "Failed to get deployment settings for file: {}",
                    &opts.deployment_file
                ))?;
            eprintln!("Deployment config: {:?}", deployment_settings);

            let mgmt_mac = match deployment_settings.deployment.mgmt_mac {
                Some(config_mac) => {
                    let mgmt_mac = FormattedMacAddress::try_from(config_mac.as_str())?;
                    eprintln!(
                        "Using mgmt_mac address found in deployment.json: {}",
                        mgmt_mac
                    );
                    mgmt_mac
                }
                None => get_ipmi_mac()?,
            };
            let generated_mac = generate_mac_address(
                &mgmt_mac,
                deployment_settings.deployment.name.as_str(),
                &NodeType::HostOS,
            )?;

            generate_network_config(&network_info, generated_mac, Path::new(&output_directory))
        }
        Some(Commands::GenerateIpv6Address { node_type }) => {
            let config_map = config_map_from_path(Path::new(&opts.config)).context(format!(
                "Failed to get config.ini settings for path: {}",
                &opts.config
            ))?;
            eprintln!("Using config: {:?}", config_map);

            let network_info = NetworkInfo::from_config_map(&config_map)?;
            eprintln!("Network info config: {:?}", &network_info);

            let deployment_settings = get_deployment_settings(Path::new(&opts.deployment_file))
                .context(format!(
                    "Failed to get deployment settings for file: {}",
                    &opts.deployment_file
                ))?;
            eprintln!("Deployment config: {:?}", deployment_settings);

            let node_type = node_type.parse::<NodeType>()?;
            let mgmt_mac = match deployment_settings.deployment.mgmt_mac {
                Some(config_mac) => {
                    let mgmt_mac = FormattedMacAddress::try_from(config_mac.as_str())?;
                    eprintln!(
                        "Using mgmt_mac address found in deployment.json: {}",
                        mgmt_mac
                    );
                    mgmt_mac
                }
                None => get_ipmi_mac()?,
            };
            let generated_mac = generate_mac_address(
                &mgmt_mac,
                deployment_settings.deployment.name.as_str(),
                &node_type,
            )?;
            let ipv6_address = generate_ipv6_address(&network_info.ipv6_prefix, &generated_mac)?;
            println!("{}", to_cidr(ipv6_address, network_info.ipv6_subnet));
            Ok(())
        }
        Some(Commands::GenerateMacAddress { node_type }) => {
            let config_map = config_map_from_path(Path::new(&opts.config)).context(format!(
                "Failed to get config.ini settings for path: {}",
                &opts.config
            ))?;
            eprintln!("Using config: {:?}", config_map);

            let network_info = NetworkInfo::from_config_map(&config_map)?;
            eprintln!("Network info config: {:?}", &network_info);

            let deployment_settings = get_deployment_settings(Path::new(&opts.deployment_file))
                .context(format!(
                    "Failed to get deployment settings for file: {}",
                    &opts.deployment_file
                ))?;
            eprintln!("Deployment config: {:?}", deployment_settings);

            let node_type = node_type.parse::<NodeType>()?;
            let mgmt_mac = match deployment_settings.deployment.mgmt_mac {
                Some(config_mac) => {
                    let mgmt_mac = FormattedMacAddress::try_from(config_mac.as_str())?;
                    eprintln!(
                        "Using mgmt_mac address found in deployment.json: {}",
                        mgmt_mac
                    );
                    mgmt_mac
                }
                None => get_ipmi_mac()?,
            };
            let generated_mac = generate_mac_address(
                &mgmt_mac,
                deployment_settings.deployment.name.as_str(),
                &node_type,
            )?;

            let generated_mac = FormattedMacAddress::from(&generated_mac);
            println!("{}", generated_mac);
            Ok(())
        }
        Some(Commands::FetchMacAddress {}) => {
            let deployment_settings = get_deployment_settings(Path::new(&opts.deployment_file))
                .context(format!(
                    "Failed to get deployment settings for file: {}",
                    &opts.deployment_file
                ))?;
            eprintln!("Deployment config: {:?}", deployment_settings);

            let mgmt_mac = match deployment_settings.deployment.mgmt_mac {
                Some(config_mac) => {
                    let mgmt_mac = FormattedMacAddress::try_from(config_mac.as_str())?;
                    eprintln!(
                        "Using mgmt_mac address found in deployment.json: {}",
                        mgmt_mac
                    );
                    mgmt_mac
                }
                None => get_ipmi_mac()?,
            };
            println!("{}", mgmt_mac);
            Ok(())
        }
        Some(Commands::RenderFirewallConfig { firewall_file }) => {
            let config = firewall_json::get_firewall_rules_json_or_default(
                firewall_file.as_ref().map(Path::new),
                Path::new(DEFAULT_HOSTOS_FIREWALL_JSON_PATH),
            )?;
            eprintln!(
                "Firewall config ({}): {:#?}",
                match firewall_file {
                    Some(f) => format!("from explicitly specified {}", f),
                    None => format!("from default {}", DEFAULT_HOSTOS_FIREWALL_JSON_PATH),
                },
                config
            );
            println!(
                "{}",
                match config {
                    Some(c) => c.as_nftables(&firewall::FirewallRuleDestination::HostOS),
                    None => "".to_string(),
                },
            );
            Ok(())
        }
        None => Err(anyhow!(
            "No subcommand specified. Run with '--help' for subcommands"
        )),
    }
}
