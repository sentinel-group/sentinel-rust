use crate::{base::SlotChain, circuitbreaker, flow, hotspot, isolation, stat, system};
use lazy_static::lazy_static;
use std::sync::Arc;

lazy_static! {
    pub static ref GLOBAL_SLOT_CHAIN : Arc<SlotChain> = {
        let mut sc = SlotChain::new();

        sc.add_stat_prepare_slot(stat::default_resource_node_prepare_slot());

        // sc.add_rule_check_slot(system::DEFAULT_ADAPTIVE_SLOT);
        sc.add_rule_check_slot(flow::default_slot());
        // sc.add_rule_check_slot(isolation::DEFAULT_SLOT);
        // sc.add_rule_check_slot(hotspot::DEFAULT_SLOT);
        // sc.add_rule_check_slot(circuitbreaker::DEFAULT_SLOT);

        sc.add_stat_slot(stat::default_resource_stat_slot());
        // sc.add_stat_slot(log::DEFAULT_LOG_SLOT);
        sc.add_stat_slot(flow::default_stand_alone_stat_slot());
        // sc.add_stat_slot(hotspot::DEFAULT_CONCURRENCY_STAT_SLOT);
        // sc.add_stat_slot(circuitbreaker::DEFAULT_METRIC_STAT_SLOT);
        Arc::new(sc)
    };
}

pub fn global_slot_chain() -> Arc<SlotChain> {
    GLOBAL_SLOT_CHAIN.clone()
}
