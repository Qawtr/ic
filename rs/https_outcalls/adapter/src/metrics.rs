use ic_metrics::MetricsRegistry;
use prometheus::{IntCounter, IntCounterVec, IntGauge};

/// Labels for request errors
pub(crate) const LABEL_BODY_RECEIVE_SIZE: &str = "body_receive_size";
pub(crate) const LABEL_HEADER_RECEIVE_SIZE: &str = "header_receive_size";
#[cfg(not(feature = "http"))]
pub(crate) const LABEL_HTTP_SCHEME: &str = "http_scheme";
pub(crate) const LABEL_HTTP_METHOD: &str = "http_method";
pub(crate) const LABEL_RESPONSE_HEADERS: &str = "response_headers";
pub(crate) const LABEL_REQUEST_HEADERS: &str = "request_headers";
pub(crate) const LABEL_CONNECT: &str = "connect";
pub(crate) const LABEL_URL_PARSE: &str = "url_parse";
pub(crate) const LABEL_UPLOAD: &str = "up";
pub(crate) const LABEL_DOWNLOAD: &str = "down";

#[derive(Clone, Debug)]
pub struct AdapterMetrics {
    /// The number of requests served by adapter.
    pub requests: IntCounter,
    /// The number of requests served via a SOCKS proxy.
    pub requests_socks: IntCounter,
    /// The number of socks connections attempts
    pub socks_connections_attempts: IntCounter,
    /// The number of socks clients in the cache
    pub socks_cache_size: IntGauge,
    /// The number of cache misses for socks clients
    pub socks_cache_miss: IntCounter,
    /// The number of successful socks connections
    pub succesful_socks_connections: IntCounterVec,
    /// Network traffic generated by adapter.
    pub network_traffic: IntCounterVec,
    /// Request failure types.
    pub request_errors: IntCounterVec,
}

impl AdapterMetrics {
    /// The constructor returns a `GossipMetrics` instance.
    pub fn new(metrics_registry: &MetricsRegistry) -> Self {
        Self {
            requests: metrics_registry.int_counter(
                "requests_total",
                "Total number of requests served by adapter",
            ),
            requests_socks: metrics_registry.int_counter(
                "requests_socks_total",
                "Total number of requests served via a SOCKS proxy",
            ),
            socks_connections_attempts: metrics_registry.int_counter(
                "socks_connections_attempts",
                "Total number of time the adapter tries to proxy a request via a SOCKS proxy",
            ),
            socks_cache_size: metrics_registry.int_gauge(
                "socks_cache_size",
                "The size of the cache for SOCKS clients",
            ),
            socks_cache_miss: metrics_registry.int_counter(
                "socks_cache_miss",
                "Total number of times the adapter failed to find a SOCKS client in the cache",
            ),
            succesful_socks_connections: metrics_registry.int_counter_vec(
                "successful_socks_connections_total",
                "Total number of successful SOCKS connections",
                &["number_of_tries"],
            ),
            network_traffic: metrics_registry.int_counter_vec(
                "network_traffic_bytes_total",
                "Network traffic generated by adapter.",
                &["link"],
            ),
            request_errors: metrics_registry.int_counter_vec(
                "request_errors_total",
                "Error types encountered in the adapter.",
                &["cause"],
            ),
        }
    }
}
