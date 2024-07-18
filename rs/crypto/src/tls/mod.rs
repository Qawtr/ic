use super::*;
use ::rustls::{ClientConfig, ServerConfig};
use ic_crypto_internal_logmon::metrics::{MetricsDomain, MetricsResult, MetricsScope};
use ic_crypto_tls_interfaces::{SomeOrAllNodes, TlsConfig, TlsConfigError, TlsPublicKeyCert};
use ic_logger::{debug, new_logger};
use ic_types::registry::RegistryClientError;
use ic_types::{NodeId, RegistryVersion};

mod rustls;
#[cfg(test)]
mod tests;

impl<CSP> TlsConfig for CryptoComponentImpl<CSP>
where
    CSP: CryptoServiceProvider + Send + Sync,
{
    fn server_config(&self, allowed_clients: SomeOrAllNodes) -> ServerConfig {
        let log_id = get_log_id(&self.logger);
        let logger = new_logger!(&self.logger;
            crypto.log_id => log_id,
            crypto.trait_name => "TlsConfig",
            crypto.method_name => "server_config",
        );
        debug!(logger;
            crypto.description => "start",
            crypto.allowed_tls_clients => format!("{:?}", allowed_clients),
        );
        let start_time = self.metrics.now();
        let result = rustls::server_handshake::server_config(
            &self.csp,
            self.node_id,
            Arc::clone(&self.registry_client),
            allowed_clients,
        );
        self.metrics.observe_duration_seconds(
            MetricsDomain::TlsConfig,
            MetricsScope::Full,
            "server_config",
            MetricsResult::Ok,
            start_time,
        );
        debug!(logger;
            crypto.description => "end",
        );
        result
    }

    fn server_config_without_client_auth(&self) -> ServerConfig {
        let log_id = get_log_id(&self.logger);
        let logger = new_logger!(&self.logger;
            crypto.log_id => log_id,
            crypto.trait_name => "TlsConfig",
            crypto.method_name => "server_config_without_client_auth",
        );
        debug!(logger;
            crypto.description => "start",
            crypto.allowed_tls_clients => "all clients allowed",
        );
        let start_time = self.metrics.now();
        let result = rustls::server_handshake::server_config_without_client_auth(
            &self.csp,
            self.node_id,
            self.registry_client.clone(),
        );
        self.metrics.observe_duration_seconds(
            MetricsDomain::TlsConfig,
            MetricsScope::Full,
            "server_config_without_client_auth",
            MetricsResult::Ok,
            start_time,
        );
        debug!(logger;
            crypto.description => "end",
        );
        result
    }

    fn client_config(&self, server: NodeId) -> ClientConfig {
        let log_id = get_log_id(&self.logger);
        let logger = new_logger!(&self.logger;
            crypto.log_id => log_id,
            crypto.trait_name => "TlsConfig",
            crypto.method_name => "client_config",
        );
        debug!(logger;
            crypto.description => "start",
            crypto.tls_server => format!("{}", server),
        );
        let start_time = self.metrics.now();
        let result = rustls::client_handshake::client_config(
            &self.csp,
            self.node_id,
            Arc::clone(&self.registry_client),
            server,
        );
        self.metrics.observe_duration_seconds(
            MetricsDomain::TlsConfig,
            MetricsScope::Full,
            "client_config",
            MetricsResult::Ok,
            start_time,
        );
        debug!(logger;
            crypto.description => "end",
        );
        result
    }
}

pub fn tls_cert_from_registry_raw(
    registry: &dyn RegistryClient,
    node_id: NodeId,
    registry_version: RegistryVersion,
) -> Result<X509PublicKeyCert, TlsCertFromRegistryError> {
    use ic_registry_client_helpers::crypto::CryptoRegistry;
    let maybe_tls_certificate = registry.get_tls_certificate(node_id, registry_version)?;
    match maybe_tls_certificate {
        None => Err(TlsCertFromRegistryError::CertificateNotInRegistry {
            node_id,
            registry_version,
        }),
        Some(cert) => Ok(cert),
    }
}

fn tls_cert_from_registry(
    registry: &dyn RegistryClient,
    node_id: NodeId,
    registry_version: RegistryVersion,
) -> Result<TlsPublicKeyCert, TlsCertFromRegistryError> {
    let raw_cert = tls_cert_from_registry_raw(registry, node_id, registry_version)?;
    TlsPublicKeyCert::try_from(raw_cert).map_err(|e| {
        TlsCertFromRegistryError::CertificateMalformed {
            internal_error: format!("{e}"),
        }
    })
}

#[derive(Debug)]
pub enum TlsCertFromRegistryError {
    RegistryError(RegistryClientError),
    CertificateNotInRegistry {
        node_id: NodeId,
        registry_version: RegistryVersion,
    },
    CertificateMalformed {
        internal_error: String,
    },
}

impl From<TlsCertFromRegistryError> for TlsConfigError {
    fn from(registry_error: TlsCertFromRegistryError) -> Self {
        match registry_error {
            TlsCertFromRegistryError::RegistryError(e) => TlsConfigError::RegistryError(e),
            TlsCertFromRegistryError::CertificateNotInRegistry {
                node_id,
                registry_version,
            } => TlsConfigError::CertificateNotInRegistry {
                node_id,
                registry_version,
            },
            TlsCertFromRegistryError::CertificateMalformed { internal_error } => {
                TlsConfigError::MalformedSelfCertificate { internal_error }
            }
        }
    }
}

impl From<RegistryClientError> for TlsCertFromRegistryError {
    fn from(registry_error: RegistryClientError) -> Self {
        TlsCertFromRegistryError::RegistryError(registry_error)
    }
}
