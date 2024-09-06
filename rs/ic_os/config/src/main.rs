use anyhow::Result;
use clap::{Parser, Subcommand};
use config::{get_config_ini_settings, get_deployment_settings, serialize_and_write_config};
use std::fs::File;
use std::path::{Path, PathBuf};

use config::types::{
    GuestOSSettings, HostOSConfig, HostOSSettings, ICOSSettings, SetupOSConfig, SetupOSSettings,
};

#[derive(Subcommand)]
pub enum Commands {
    /// Creates SetupOSConfig object
    CreateSetuposConfig {
        #[arg(long, default_value_t = config::DEFAULT_SETUPOS_CONFIG_FILE_PATH.to_string(), value_name = "config.ini")]
        config_ini_path: String,

        #[arg(long, default_value_t = config::DEFAULT_SETUPOS_DEPLOYMENT_JSON_PATH.to_string(), value_name = "deployment.json")]
        deployment_json_path: String,

        #[arg(long, default_value_t = config::DEFAULT_SETUPOS_NNS_PUBLIC_KEY_PATH.to_string(), value_name = "nns_public_key.pem")]
        nns_public_key_path: String,

        #[arg(long, default_value_t = config::DEFAULT_SETUPOS_SSH_AUTHORIZED_KEYS_PATH.to_string(), value_name = "ssh_authorized_keys")]
        ssh_authorized_keys_path: String,

        #[arg(long, default_value_t = config::DEFAULT_SETUPOS_NODE_OPERATOR_PRIVATE_KEY_PATH.to_string(), value_name = "node_operator_private_key.pem")]
        node_operator_private_key_path: String,

        #[arg(long, default_value_t = config::DEFAULT_SETUPOS_CONFIG_OBJECT_PATH.to_string(), value_name = "config.json")]
        setupos_config_json_path: String,
    },
    /// Creates HostOSConfig object from existing SetupOS config.json file
    GenerateHostosConfig {
        #[arg(long, default_value_t = config::DEFAULT_SETUPOS_CONFIG_OBJECT_PATH.to_string(), value_name = "config.json")]
        setupos_config_json_path: String,
    },
}

#[derive(Parser)]
#[command()]
struct ConfigArgs {
    #[command(subcommand)]
    command: Option<Commands>,
}

pub fn main() -> Result<()> {
    let opts = ConfigArgs::parse();

    match opts.command {
        Some(Commands::CreateSetuposConfig {
            config_ini_path,
            deployment_json_path,
            nns_public_key_path,
            ssh_authorized_keys_path,
            node_operator_private_key_path,
            setupos_config_json_path,
        }) => {
            let config_ini_path = Path::new(&config_ini_path);
            let deployment_json_path = Path::new(&deployment_json_path);
            let nns_public_key_path = Path::new(&nns_public_key_path);
            let ssh_authorized_keys_path = Some(PathBuf::from(ssh_authorized_keys_path));

            let node_operator_private_key_path = PathBuf::from(&node_operator_private_key_path);
            let node_operator_private_key_path = node_operator_private_key_path
                .exists()
                .then_some(node_operator_private_key_path);

            // get config.ini variables
            let (network_settings, verbose) = get_config_ini_settings(config_ini_path)?;

            // get deployment.json variables
            let (vm_memory, vm_cpu, nns_urls, hostname, elasticsearch_hosts) =
                get_deployment_settings(deployment_json_path);

            let icos_settings = ICOSSettings {
                nns_public_key_path: nns_public_key_path.to_path_buf(),
                nns_urls,
                elasticsearch_hosts,
                elasticsearch_tags: None,
                hostname,
                node_operator_private_key_path,
                ssh_authorized_keys_path,
            };

            let setupos_settings = SetupOSSettings;

            let hostos_settings = HostOSSettings {
                vm_memory,
                vm_cpu,
                verbose,
            };

            let guestos_settings = GuestOSSettings::default();

            let setupos_config = SetupOSConfig {
                network_settings,
                icos_settings,
                setupos_settings,
                hostos_settings,
                guestos_settings,
            };

            let setupos_config_json_path = Path::new(&setupos_config_json_path);
            serialize_and_write_config(setupos_config_json_path, &setupos_config)?;

            println!(
                "SetuposConfig has been written to {}",
                setupos_config_json_path.display()
            );

            Ok(())
        }
        Some(Commands::GenerateHostosConfig {
            setupos_config_json_path,
        }) => {
            let setupos_config_json_path = Path::new(&setupos_config_json_path);

            let setupos_config: SetupOSConfig =
                serde_json::from_reader(File::open(setupos_config_json_path)?)?;

            let hostos_config = HostOSConfig {
                network_settings: setupos_config.network_settings,
                icos_settings: setupos_config.icos_settings,
                hostos_settings: setupos_config.hostos_settings,
                guestos_settings: setupos_config.guestos_settings,
            };

            let hostos_config_output_path = Path::new("/var/ic/config/config-hostos.json");
            serialize_and_write_config(hostos_config_output_path, &hostos_config)?;

            println!(
                "HostOSConfig has been written to {}",
                hostos_config_output_path.display()
            );

            Ok(())
        }
        None => Ok(()),
    }
}
