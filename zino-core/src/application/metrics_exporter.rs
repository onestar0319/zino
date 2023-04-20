use super::Application;
use crate::extension::TomlTableExt;
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder};
use std::{net::IpAddr, time::Duration};

/// Initializes the metrics exporters.
pub(super) fn init<APP: Application + ?Sized>() {
    if let Some(metrics) = APP::config().get_table("metrics") {
        let exporter = metrics.get_str("exporter").unwrap_or_default();
        if exporter == "prometheus" {
            let mut builder = if let Some(endpoint) = metrics.get_str("push-gateway") {
                let interval = metrics
                    .get_duration("interval")
                    .unwrap_or_else(|| Duration::from_secs(60));
                PrometheusBuilder::new()
                    .with_push_gateway(endpoint, interval)
                    .expect("fail to configure the exporter to run in push gateway mode")
            } else {
                let host = metrics.get_str("host").unwrap_or("127.0.0.1");
                let port = metrics.get_u16("port").unwrap_or(9000);
                let host_addr = host
                    .parse::<IpAddr>()
                    .unwrap_or_else(|err| panic!("invalid host address `{host}`: {err}"));
                tracing::warn!(exporter, "listen on {host_addr}:{port}");
                PrometheusBuilder::new().with_http_listener((host_addr, port))
            };
            if let Some(quantiles) = metrics.get_array("quantiles") {
                let quantiles = quantiles
                    .iter()
                    .filter_map(|q| q.as_float())
                    .collect::<Vec<_>>();
                builder = builder
                    .set_quantiles(&quantiles)
                    .expect("invalid quantiles to render histograms");
            }
            if let Some(buckets) = metrics.get_table("buckets") {
                for (key, value) in buckets {
                    let matcher = if key.starts_with('^') {
                        Matcher::Prefix(key.to_owned())
                    } else if key.ends_with('$') {
                        Matcher::Suffix(key.to_owned())
                    } else {
                        Matcher::Full(key.to_owned())
                    };
                    let values = value
                        .as_array()
                        .expect("buckets should be an array of floats")
                        .iter()
                        .filter_map(|v| v.as_float())
                        .collect::<Vec<_>>();
                    builder = builder
                        .set_buckets_for_metric(matcher, &values)
                        .expect("invalid buckets to render histograms");
                }
            }
            if let Some(labels) = metrics.get_table("global-labels") {
                for (key, value) in labels {
                    builder = builder.add_global_label(key, value.to_string());
                }
            }
            if let Some(addresses) = metrics.get_array("allowed-addresses") {
                for addr in addresses {
                    builder = builder
                        .add_allowed_address(addr.as_str().unwrap_or_default())
                        .unwrap_or_else(|err| panic!("invalid IP address `{addr}`: {err}"));
                }
            }
            builder
                .install()
                .expect("fail to install Prometheus exporter");
        } else {
            tracing::error!("metrics exporter `{exporter}` is unsupported");
        }
    }
}
