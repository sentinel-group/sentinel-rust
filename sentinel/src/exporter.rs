///! exporter the process protected by Sentinel
use lazy_static::lazy_static;
use prometheus_exporter::{
    Builder,
    prometheus::{default_registry, opts, Gauge, GaugeVec, Registry}};
use std::sync::Once;
use crate::config;

lazy_static! {
    static ref HOST_NAME: String = hostname::get().unwrap().into_string().unwrap();
    static ref PROCESS_NAME: String = std::env::args().collect::<Vec<String>>()[0].clone();
    static ref PID_STRING: String = format!("{}", std::process::id());
    static ref CPU_RATIO: GaugeVec = GaugeVec::new(
        opts!(
            "sentinel_process_cpu_ratio",
            "current process cpu utilization ratio"
        ),
        &["host", "process", "cpu", "process_cpu_ratio"]
    )
    .unwrap();
    static ref PROCESS_MEMORY_SIZE: GaugeVec = GaugeVec::new(
        opts!(
            "sentinel_process_memory_size",
            "current process memory size in bytes"
        ),
        &["host", "process", "pid", "total_memory_size"]
    )
    .unwrap();
    static ref RESOURCE_FLOW_THRESHOLD: GaugeVec = GaugeVec::new(
        opts!(
            "sentinel_resource_flow_threshold",
            "resource flow threshold"
        ),
        &["host", "resource", "threshold"]
    )
    .unwrap();
    static ref METRICS: Vec<GaugeVec> = {
        let mut vec = Vec::<GaugeVec>::new();
        vec.push(CPU_RATIO.clone());
        vec.push(PROCESS_MEMORY_SIZE.clone());
        vec.push(RESOURCE_FLOW_THRESHOLD.clone());
        vec
    };
    static ref INIT_ONCE: Once = Once::new();
}

pub fn set_cpu_ratio(percent: f32) {
    CPU_RATIO
        .with_label_values(&[&HOST_NAME, &PROCESS_NAME, &PID_STRING, "process_cpu_ratio"])
        .set(percent as f64);
}

pub fn set_process_memory_size(mem_size: u64) {
    PROCESS_MEMORY_SIZE
        .with_label_values(&[&HOST_NAME, &PROCESS_NAME, &PID_STRING, "total_memory_size"])
        .set(mem_size as f64);
}

pub fn set_resource_flow_threshold(resourse: String, threshold: f64) {
    RESOURCE_FLOW_THRESHOLD
        .with_label_values(&[&HOST_NAME, &format!("rs:{}", resourse), "threshold"])
        .set(threshold);
}

fn register_sentinel_metrics(registry: Option<Box<Registry>>) {
    let r = match registry {
        Some(ref r) => r,
        None => default_registry(),
    };
    for item in &*METRICS {
        r.register(Box::new(item.clone())).unwrap();
    }
}

pub fn reset_sentinel_metrics() {
    for item in &*METRICS {
        item.reset()
    }
}


pub fn init(){
    INIT_ONCE.call_once(move || {
        // currently, `prometheus_exporter` crate only support global registry
        register_sentinel_metrics(None); 
        let binding = config::exporter_addr().parse().unwrap();
        let metrics_path = config::exporter_metrics_path();
        let mut builder = Builder::new(binding);
        builder.with_endpoint(&metrics_path);
        builder.start().unwrap();
    });
}