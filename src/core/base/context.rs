//! Context
//!
use super::{ResourceWrapper, SentinelEntry, StatNode, TokenResult};
use crate::utils::time::curr_time_millis;
use std::any::Any;
use std::collections::HashMap;
use std::rc::Weak;
use std::sync::Arc;

#[derive(Default)]
pub struct EntryContext {
    /// entry and context do not need to be `Send/Sync`
    /// entry<->context, cycled reference, so need Weak
    /// context should not change entry, so here we do not use RefCell
    entry: Option<Weak<SentinelEntry>>,
    /// Use to calculate RT
    start_time: u64,
    /// The round trip time of this transaction
    round_trip: u64,
    resource: ResourceWrapper,
    // todo: is it neccessary to keep using trait object here?
    // consider replacing by `crate::core::stat::ResourceNode`
    stat_node: Option<Arc<dyn StatNode>>,
    input: SentinelInput,
    /// the result of rule slots check
    rule_check_result: TokenResult,
}

impl EntryContext {
    pub fn new() -> Self {
        EntryContext {
            start_time: curr_time_millis(),
            ..Default::default()
        }
    }

    pub fn set_entry(&mut self, entry: Weak<SentinelEntry>) {
        self.entry = Some(entry);
    }

    pub fn entry(&self) -> Option<Weak<SentinelEntry>> {
        self.entry.clone()
    }

    pub fn start_time(&self) -> u64 {
        self.start_time
    }

    pub fn is_blocked(&self) -> bool {
        self.rule_check_result.is_blocked()
    }

    pub fn set_round_trip(&mut self, round_trip: u64) {
        self.round_trip = round_trip
    }

    pub fn round_trip(&self) -> u64 {
        self.round_trip
    }

    pub fn set_resource(&mut self, resource: ResourceWrapper) {
        self.resource = resource;
    }

    pub fn resource(&self) -> &ResourceWrapper {
        &self.resource
    }

    pub fn set_input(&mut self, input: SentinelInput) {
        self.input = input;
    }

    pub fn input(&self) -> &SentinelInput {
        &self.input
    }

    pub fn set_stat_node(&mut self, stat_node: Arc<dyn StatNode>) {
        self.stat_node = Some(stat_node);
    }

    pub fn stat_node(&self) -> Option<Arc<dyn StatNode>> {
        self.stat_node.clone()
    }

    pub fn set_result(&mut self, result: TokenResult) {
        self.rule_check_result = result;
    }

    pub fn reset_result_to_pass(&mut self) {
        self.rule_check_result.reset_to_pass();
    }

    pub fn result(&self) -> &TokenResult {
        &self.rule_check_result
    }
}

#[derive(Debug)]
pub struct SentinelInput {
    batch_count: u32,
    flag: i32,
    args: Option<Vec<Arc<dyn Any>>>,
    attachments: Option<HashMap<Arc<dyn Any>, Arc<dyn Any>>>,
}

impl Default for SentinelInput {
    fn default() -> Self {
        SentinelInput {
            batch_count: 1,
            flag: 0,
            args: None,
            attachments: None,
        }
    }
}

impl SentinelInput {
    pub fn new(batch_count: u32, flag: i32) -> Self {
        SentinelInput {
            batch_count,
            flag,
            ..Default::default()
        }
    }

    pub fn set_batch_count(&mut self, batch_count: u32) {
        self.batch_count = batch_count;
    }

    pub fn batch_count(&self) -> u32 {
        self.batch_count
    }

    pub fn set_flag(&mut self, flag: i32) {
        self.flag = flag;
    }

    pub fn flag(&self) -> i32 {
        self.flag
    }

    pub fn set_args(&mut self, args: Vec<Arc<dyn Any>>) {
        self.args = Some(args);
    }

    pub fn args(&self) -> Option<&Vec<Arc<dyn Any>>> {
        self.args.as_ref()
    }

    pub fn set_attachments(&mut self, attachments: HashMap<Arc<dyn Any>, Arc<dyn Any>>) {
        self.attachments = Some(attachments);
    }

    pub fn attachments(&self) -> Option<&HashMap<Arc<dyn Any>, Arc<dyn Any>>> {
        self.attachments.as_ref()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::core::base::result::BlockType;
    #[test]
    fn is_blocked() {
        let mut ctx = EntryContext::new();
        assert_eq!(ctx.is_blocked(), false);
        ctx.set_result(TokenResult::new_blocked(BlockType::Other(1)));
        assert_eq!(ctx.is_blocked(), true);
    }
}
