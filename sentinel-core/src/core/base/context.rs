//! Context
//!
use super::{EntryWeakPtr, ResourceWrapper, StatNode, TokenResult};
use crate::utils::time::curr_time_millis;
use crate::Error;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
pub type ContextPtr = Arc<RwLock<EntryContext>>;

#[derive(Default)]
pub struct EntryContext {
    /// entry<->context, cycled reference, so need Weak,
    /// context should not change entry, so here we do not use RwLock,
    /// todo: do we need this N:M mapping from context to entry, consider 1:1 mapping?
    entry: Option<EntryWeakPtr>,
    /// Use to calculate RT
    start_time: u64,
    /// The round trip time of this transaction
    round_trip: u64,
    resource: ResourceWrapper,
    // todo: is it necessary to keep using trait object here?
    // consider replacing by `crate::core::stat::ResourceNode`
    stat_node: Option<Arc<dyn StatNode>>,
    input: SentinelInput,
    /// the result of rule slots check
    rule_check_result: TokenResult,
    err: Option<Error>,
}

impl EntryContext {
    pub fn new() -> Self {
        EntryContext {
            start_time: curr_time_millis(),
            ..Default::default()
        }
    }

    pub fn set_entry(&mut self, entry: EntryWeakPtr) {
        self.entry = Some(entry);
    }

    pub fn entry(&self) -> Option<&EntryWeakPtr> {
        self.entry.as_ref()
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

    pub fn set_err(&mut self, err: Error) {
        self.err = Some(err);
    }

    pub fn get_err(&self) -> &Option<Error> {
        &self.err
    }
}

pub type ParamKey = String;
pub type ParamsList = Vec<ParamKey>;
pub type ParamsMap = HashMap<String, ParamKey>;

/// Input of policy algorithms
#[derive(Debug)]
pub struct SentinelInput {
    batch_count: u32,
    flag: i32,
    /// following input items are used in hotspot module
    args: Option<ParamsList>,
    attachments: Option<ParamsMap>,
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

    pub fn set_args(&mut self, args: ParamsList) {
        self.args = Some(args);
    }

    pub fn args(&self) -> Option<&ParamsList> {
        self.args.as_ref()
    }

    pub fn set_attachments(&mut self, attachments: ParamsMap) {
        self.attachments = Some(attachments);
    }

    pub fn attachments(&self) -> Option<&ParamsMap> {
        self.attachments.as_ref()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::base::result::BlockType;
    #[test]
    fn is_blocked() {
        let mut ctx = EntryContext::new();
        assert!(!ctx.is_blocked());
        ctx.set_result(TokenResult::new_blocked(BlockType::Other(1)));
        assert!(ctx.is_blocked());
    }
}
