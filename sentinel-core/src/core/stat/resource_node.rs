use super::{BucketLeapArray, SlidingWindowMetric};
use crate::{
    base::{
        ConcurrencyStat, MetricEvent, MetricItem, MetricItemRetriever, ReadStat, ResourceType,
        StatNode, TimePredicate, WriteStat,
    },
    config, Result,
};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

#[allow(dead_code)]
#[derive(Debug)]
pub struct ResourceNode {
    pub(crate) res_name: String,
    pub(crate) resource_type: ResourceType,
    pub(crate) sample_count: u32,
    pub(crate) interval_ms: u32,
    pub(crate) concurrency: AtomicU32,
    pub(crate) arr: Arc<BucketLeapArray>,
    pub(crate) metric: Arc<SlidingWindowMetric>,
}

impl ResourceNode {
    pub fn new(res_name: String, resource_type: ResourceType) -> Self {
        let arr = Arc::new(
            BucketLeapArray::new(
                config::global_stat_sample_count_total(),
                config::global_stat_interval_ms_total(),
            )
            .unwrap(),
        );
        let sample_count = config::metric_stat_sample_count();
        let interval_ms = config::metric_stat_interval_ms();
        let metric =
            Arc::new(SlidingWindowMetric::new(sample_count, interval_ms, arr.clone()).unwrap());
        ResourceNode {
            res_name,
            resource_type,
            sample_count,
            interval_ms,
            concurrency: AtomicU32::new(0),
            arr,
            metric,
        }
    }

    pub fn default_metric(&self) -> Arc<dyn ReadStat> {
        self.metric.clone()
    }

    pub fn max_avg(&self, event: MetricEvent) -> f64 {
        self.metric.max_of_single_bucket(event) as f64 * self.sample_count as f64
            / self.interval_ms as f64
            * 1000f64
    }

    pub fn max_concurrency(&self) -> u32 {
        self.metric.max_concurrency()
    }
}

impl MetricItemRetriever for ResourceNode {
    fn metrics_on_condition(&self, predicate: &TimePredicate) -> Vec<MetricItem> {
        self.metric.second_metrics_on_condition(predicate)
    }
}

impl ReadStat for ResourceNode {
    fn qps(&self, event: MetricEvent) -> f64 {
        self.metric.qps(event)
    }
    fn qps_previous(&self, event: MetricEvent) -> f64 {
        self.metric.qps_previous(event)
    }
    fn sum(&self, event: MetricEvent) -> u64 {
        self.metric.sum(event)
    }
    fn min_rt(&self) -> f64 {
        self.metric.min_rt()
    }
    fn avg_rt(&self) -> f64 {
        self.metric.avg_rt()
    }
}

impl WriteStat for ResourceNode {
    fn add_count(&self, event: MetricEvent, count: u64) {
        self.arr.add_count(event, count);
    }

    fn update_concurrency(&self, concurrency: u32) {
        self.arr.update_concurrency(concurrency);
    }
}

impl ConcurrencyStat for ResourceNode {
    fn current_concurrency(&self) -> u32 {
        self.concurrency.load(Ordering::SeqCst)
    }

    fn increase_concurrency(&self) {
        self.arr
            .update_concurrency(self.concurrency.fetch_add(1, Ordering::SeqCst) + 1)
    }

    fn decrease_concurrency(&self) {
        self.concurrency.fetch_sub(1, Ordering::SeqCst);
    }
}

impl StatNode for ResourceNode {
    fn generate_read_stat(&self, sample_count: u32, interval_ms: u32) -> Result<Arc<dyn ReadStat>> {
        let stat = SlidingWindowMetric::new(sample_count, interval_ms, self.arr.clone())?;
        Ok(Arc::new(stat))
    }
}
