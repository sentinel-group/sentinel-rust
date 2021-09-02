use crate::{base::SlotChain, circuitbreaker, flow, hotspot, isolation, stat, system};
use lazy_static::lazy_static;
use std::sync::Arc;

lazy_static! {
    pub static ref GLOBAL_SLOT_CHAIN: Arc<SlotChain> = {
        let mut sc = SlotChain::new();

        sc.add_stat_prepare_slot(stat::default_resource_node_prepare_slot());

        sc.add_rule_check_slot(system::default_slot()); // 1000
        sc.add_rule_check_slot(flow::default_slot()); // 2000
        sc.add_rule_check_slot(isolation::default_slot()); // 3000
        sc.add_rule_check_slot(hotspot::default_slot()); // 4000
        sc.add_rule_check_slot(circuitbreaker::default_slot()); // 5000

        sc.add_stat_slot(stat::default_resource_stat_slot()); // 1000
        sc.add_stat_slot(crate::log::default_stat_slot()); // 2000
        sc.add_stat_slot(flow::default_stand_alone_stat_slot()); // 3000
        sc.add_stat_slot(hotspot::default_stand_alone_stat_slot()); // 4000
        sc.add_stat_slot(circuitbreaker::default_metric_stat_slot()); // 5000
        Arc::new(sc)
    };
}

pub fn global_slot_chain() -> Arc<SlotChain> {
    GLOBAL_SLOT_CHAIN.clone()
}
