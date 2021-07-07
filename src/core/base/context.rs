//! Context
//!
use super::{ResourceWrapper, SentinelEntry, StatNode, TokenResult};
use crate::utils::time::curr_time_millis;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Default)]
pub struct EntryContext {
    pub(crate) entry: Option<Rc<RefCell<SentinelEntry>>>,
    /// Use to calculate RT
    pub(crate) start_time: u64,
    /// The rt of this transaction
    pub(crate) rt: u64,
    pub(crate) res: Option<ResourceWrapper>,
    pub(crate) stat_node: Option<Rc<RefCell<dyn StatNode>>>,
    pub(crate) input: Option<SentinelInput>,
    /// the result of rule slots check
    pub(crate) rule_check_result: TokenResult,
    /// reserve for storing some intermediate data from the Entry execution process
    pub(crate) data: HashMap<Rc<RefCell<dyn Any>>, Rc<RefCell<dyn Any>>>,
}

impl EntryContext {
    pub fn new() -> Self {
        Self {
            start_time: curr_time_millis(),
            ..Self::default()
        }
    }

    pub fn set_entry(&mut self, entry: Rc<RefCell<SentinelEntry>>) {
        self.entry = Some(entry);
    }

    pub fn entry(&self) -> Option<Rc<RefCell<SentinelEntry>>> {
        self.entry.clone()
    }

    pub fn start_time(&self) -> u64 {
        self.start_time
    }

    pub fn is_blocked(&self) -> bool {
        self.rule_check_result.is_blocked()
    }

    pub fn set_rt(&mut self, rt: u64) {
        self.rt = rt
    }

    pub fn rt(&self) -> u64 {
        self.rt
    }
}

#[derive(Debug)]
pub struct SentinelInput {
    pub(crate) batch_count: u32,
    pub(crate) flag: i32,
    pub(crate) args: Vec<Rc<RefCell<dyn Any>>>,
    pub(crate) attachments: HashMap<Rc<RefCell<dyn Any>>, Rc<RefCell<dyn Any>>>,
}

impl Default for SentinelInput {
    fn default() -> Self {
        SentinelInput {
            batch_count: 1,
            flag: 0,
            args: Vec::new(),
            attachments: HashMap::new(),
        }
    }
}

impl SentinelInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = SentinelInput::default();
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
        ctx.rule_check_result = TokenResult::new_blocked(BlockType::Other(1));
        assert_eq!(ctx.is_blocked(), true);
    }
}
