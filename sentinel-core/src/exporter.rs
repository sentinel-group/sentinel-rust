use crate::{
    base::{BlockType, TokenResult},
    config,
};
///! exporter the process protected by Sentinel
use lazy_static::lazy_static;
use prometheus_exporter::{
    prometheus::{default_registry, opts, CounterVec, GaugeVec, Registry},
    Builder,
};
use std::sync::Once;
use sysinfo::{System, SystemExt};

lazy_static! {
    static ref HOST_NAME: String = System::new().host_name().unwrap_or_else(|| "<unknown>".to_owned());
    static ref PROCESS_NAME: String = std::env::args().collect::<Vec<String>>()[0].clone();
    static ref PID_STRING: String = format!("{}", std::process::id());
    // crate::core::system_metric
    static ref CPU_RATIO_GAUGE: GaugeVec = GaugeVec::new(
        opts!(
            "sentinel_process_CPU_RATIO_GAUGE",
            "current process cpu utilization ratio"
        ),
        &["host", "process", "pid"]
    )
    .unwrap();
    static ref MEMORY_SIZE_GAUGE: GaugeVec = GaugeVec::new(
        opts!(
            "sentinel_MEMORY_SIZE_GAUGE",
            "current process memory size in bytes"
        ),
        &["host", "process", "pid"]
    )
    .unwrap();
    // crate::core::flow
    static ref FLOW_THRESHOLD_GAUGE: GaugeVec = GaugeVec::new(
        opts!(
            "sentinel_FLOW_THRESHOLD_GAUGE",
            "resource flow threshold"
        ),
        &["host", "process", "pid", "resource"]
    )
    .unwrap();
    // crate::core::circuitbreaker
    static ref STATE_CHANGE_COUNTER: CounterVec = CounterVec::new(
        opts!(
            "circuit_breaker_state_changed_total",
            "Circuit breaker total state change count"
        ),
        &["host", "process", "pid", "resource","from_state","to_state"]
    )
    .unwrap();
    // crate::core::stat
    static ref HANDLED_COUNTER: CounterVec = CounterVec::new(
        opts!(
            "handled_total",
            "Total handled count"
        ),
        &["host", "process", "pid", "resource","result","block_type"]
    )
    .unwrap();
    static ref GAUGE_METRICS: Vec<GaugeVec> = {
        vec![CPU_RATIO_GAUGE.clone(), MEMORY_SIZE_GAUGE.clone(), FLOW_THRESHOLD_GAUGE.clone()]
    };
    static ref COUNTER_METRICS: Vec<CounterVec> = {
        vec![STATE_CHANGE_COUNTER.clone(), HANDLED_COUNTER.clone()]
    };
    static ref INIT_ONCE: Once = Once::new();
}

pub fn set_cpu_ratio(percent: f32) {
    CPU_RATIO_GAUGE
        .with_label_values(&[&HOST_NAME, &PROCESS_NAME, &PID_STRING])
        .set(percent as f64);
}

pub fn set_memory_size(mem_size: u64) {
    MEMORY_SIZE_GAUGE
        .with_label_values(&[&HOST_NAME, &PROCESS_NAME, &PID_STRING])
        .set(mem_size as f64);
}

pub fn set_flow_threshold(resourse: &str, threshold: f64) {
    FLOW_THRESHOLD_GAUGE
        .with_label_values(&[&HOST_NAME, &PROCESS_NAME, &PID_STRING, resourse])
        .set(threshold);
}

pub fn add_state_change_counter(resourse: &str, from: &str, to: &str) {
    STATE_CHANGE_COUNTER
        .with_label_values(&[&HOST_NAME, &PROCESS_NAME, &PID_STRING, resourse, from, to])
        .inc_by(1.0);
}

pub fn add_handled_counter(
    batch_count: u32,
    resource: &str,
    result: TokenResult,
    block_type: Option<BlockType>,
) {
    HANDLED_COUNTER
        .with_label_values(&[
            &HOST_NAME,
            &PROCESS_NAME,
            &PID_STRING,
            resource,
            &result.to_string(),
            &block_type.map_or(String::new(), |v| v.to_string()),
        ])
        .inc_by(batch_count as f64);
}

fn register_sentinel_metrics(registry: Option<Box<Registry>>) {
    let r = match registry {
        Some(ref r) => r,
        None => default_registry(),
    };
    for item in &*GAUGE_METRICS {
        r.register(Box::new(item.clone())).unwrap();
    }
    for item in &*COUNTER_METRICS {
        r.register(Box::new(item.clone())).unwrap();
    }
}

pub fn reset_sentinel_metrics() {
    for item in &*GAUGE_METRICS {
        item.reset();
    }
    for item in &*COUNTER_METRICS {
        item.reset();
    }
}

pub fn init() {
    INIT_ONCE.call_once(move || {
        // currently, `prometheus_exporter` crate only support global registry
        register_sentinel_metrics(None);
        let binding = config::exporter_addr().parse().unwrap();
        let metrics_path = config::exporter_metrics_path();
        let mut builder = Builder::new(binding);
        builder.with_endpoint(&metrics_path).unwrap();
        builder.start().unwrap();
    });
}
