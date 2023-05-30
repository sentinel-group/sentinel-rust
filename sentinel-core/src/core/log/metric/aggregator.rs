use super::*;
use crate::{
    base::{MetricItem, MetricItemRetriever},
    config, logging,
    stat::{self, ResourceNode},
    utils::sleep_for_ms,
};
use lazy_static::lazy_static;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Once};

lazy_static! {
    /// The timestamp of the last fetching. The time unit is ms (= second * 1000).
    static ref LAST_FETCH_TIME: AtomicU64 = AtomicU64::new(0);
    static ref METRIC_WRITER: Option<Mutex<DefaultMetricLogWriter>> = {
        let flush_interval = config::metric_log_flush_interval_sec();
        if flush_interval == 0 {
            return None
        }
        match DefaultMetricLogWriter::new(config::metric_log_single_file_max_size(), config::metric_log_max_file_amount()){
            Ok(writer) => Some(Mutex::new(writer)),
            Err(err) => {
                logging::error!("Failed to initialize the MetricLogWriter in aggregator::init_task(). Error: {:?}", err);
                None
            }
        }
    };
    static ref INIT_ONCE : Once = Once::new();
}

pub fn init_task() {
    INIT_ONCE.call_once(|| {
        std::thread::spawn(|| loop {
            do_aggregate();
            sleep_for_ms((config::metric_log_flush_interval_sec() * 1000).into());
        });
    });
}

pub fn write_task(mut map: MetricTimeMap) {
    let mut keys = Vec::with_capacity(map.len());
    for (k, _) in map.iter() {
        keys.push(*k);
    }
    // Sort the time
    keys.sort_unstable();
    println!("keys: {:?}", keys);

    let writer = METRIC_WRITER.as_ref().unwrap();
    let mut writer = writer.lock().unwrap();
    for k in keys {
        writer.write(k, &mut *map.entry(k).or_default()).unwrap_or_else(|err|{
			logging::error!("[MetricAggregatorTask] fail to write metric in aggregator::write_task(). Error: {:?}",err);});
    }
}

pub fn do_aggregate() {
    let mut cur_time = crate::utils::curr_time_millis();
    cur_time = cur_time - cur_time % 1000;

    if cur_time <= LAST_FETCH_TIME.load(Ordering::SeqCst) {
        return;
    }
    let mut map = MetricTimeMap::new();
    let cns = stat::resource_node_list();
    for node in cns {
        let metrics = current_metric_items(Arc::clone(&node), cur_time);
        aggregate_into_map(&mut map, metrics, Arc::clone(&node));
    }
    // Aggregate for inbound entrance node.
    aggregate_into_map(
        &mut map,
        current_metric_items(stat::inbound_node(), cur_time),
        stat::inbound_node(),
    );
    println!("map: {:?}", map);

    // Update current last fetch timestamp.
    LAST_FETCH_TIME.store(cur_time, Ordering::SeqCst);

    println!("Store last fetch time {}", cur_time);

    if !map.is_empty() {
        std::thread::spawn(move || write_task(map));
    }
}

fn aggregate_into_map(
    mm: &mut MetricTimeMap,
    metrics: HashMap<u64, MetricItem>,
    node: Arc<ResourceNode>,
) {
    for (t, mut item) in metrics {
        item.resource = node.res_name.clone();
        item.resource_type = node.resource_type;
        if let std::collections::hash_map::Entry::Vacant(e) = mm.entry(t) {
            e.insert(vec![item]);
        } else {
            mm.entry(t).or_default().push(item);
        }
    }
}

fn is_active_metric_item(item: &MetricItem) -> bool {
    item.pass_qps > 0
        || item.block_qps > 0
        || item.complete_qps > 0
        || item.error_qps > 0
        || item.avg_rt > 0
        || item.concurrency > 0
}

fn is_item_time_stamp_in_time(ts: u64, current_sec_start: u64) -> bool {
    // The bucket should satisfy: windowStart between [LAST_FETCH_TIME, current_sec_start)
    ts >= LAST_FETCH_TIME.load(Ordering::SeqCst) && ts < current_sec_start
}

fn current_metric_items<T: MetricItemRetriever>(
    retriever: Arc<T>,
    current_time: u64,
) -> HashMap<u64, MetricItem> {
    let items = retriever.metrics_on_condition(&move |ts: u64| -> bool {
        is_item_time_stamp_in_time(ts, current_time)
    });
    let mut m = HashMap::with_capacity(items.len());
    for item in items {
        if !is_active_metric_item(&item) {
            continue;
        }
        m.insert(item.timestamp, item);
    }
    m
}
