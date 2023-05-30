//! Result
//!
use super::{BlockError, SentinelRule, Snapshot};
use crate::{Error, Result};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

type OtherBlockType = u8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlockType {
    Unknown,
    Flow,
    Isolation,
    CircuitBreaking,
    SystemFlow,
    HotSpotParamFlow,
    Other(OtherBlockType),
}

impl Default for BlockType {
    fn default() -> Self {
        Self::Unknown
    }
}

lazy_static! {
    static ref BLOCK_TYPE_MAP: Mutex<HashMap<OtherBlockType, &'static str>> =
        Mutex::new(HashMap::new());
}

const EXIST_BLOCK_ERROR: &str = "Block type existed!";

pub fn registry_block_type(other: BlockType, desc: &'static str) -> Result<()> {
    match other {
        BlockType::Other(id) => {
            if let std::collections::hash_map::Entry::Vacant(e) =
                BLOCK_TYPE_MAP.lock().unwrap().entry(id)
            {
                e.insert(desc);
                Ok(())
            } else {
                Err(Error::msg(EXIST_BLOCK_ERROR))
            }
        }
        _ => Err(Error::msg(EXIST_BLOCK_ERROR)),
    }
}

impl fmt::Display for BlockType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let BlockType::Other(id) = self {
            match BLOCK_TYPE_MAP.lock().unwrap().get(id) {
                Some(&desc) => write!(f, "{}", desc),
                None => write!(f, "{}", id),
            }
        } else {
            write!(f, "{:?}", self)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenResult {
    Pass,
    Blocked(BlockError),
    Wait(u64),
}

impl Default for TokenResult {
    fn default() -> Self {
        TokenResult::Pass
    }
}

impl TokenResult {
    pub fn new_pass() -> Self {
        Self::default()
    }

    pub fn new_should_wait(nanos_to_wait: u64) -> Self {
        Self::Wait(nanos_to_wait)
    }

    pub fn new_blocked(block_type: BlockType) -> Self {
        Self::Blocked(BlockError::new(block_type))
    }

    pub fn new_blocked_with_msg(block_type: BlockType, block_msg: String) -> Self {
        Self::Blocked(BlockError::new_with_msg(block_type, block_msg))
    }

    pub fn new_blocked_with_cause(
        block_type: BlockType,
        block_msg: String,
        rule: Arc<dyn SentinelRule>,
        snapshot_value: Arc<Snapshot>,
    ) -> Self {
        Self::Blocked(BlockError::new_with_cause(
            block_type,
            block_msg,
            rule,
            snapshot_value,
        ))
    }

    pub fn reset_to_pass(&mut self) {
        *self = Self::new_pass();
    }

    pub fn reset_to_blocked(&mut self, block_type: BlockType) {
        *self = Self::new_blocked(block_type);
    }

    pub fn reset_to_blocked_with_msg(&mut self, block_type: BlockType, block_msg: String) {
        *self = Self::new_blocked_with_msg(block_type, block_msg);
    }

    pub fn reset_to_blocked_with_cause(
        &mut self,
        block_type: BlockType,
        block_msg: String,
        rule: Arc<dyn SentinelRule>,
        snapshot_value: Arc<Snapshot>,
    ) {
        *self = Self::new_blocked_with_cause(block_type, block_msg, rule, snapshot_value);
    }

    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass)
    }

    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Blocked(_))
    }

    pub fn is_wait(&self) -> bool {
        matches!(self, Self::Wait(_))
    }

    pub fn block_err(&self) -> Option<BlockError> {
        match self {
            Self::Blocked(err) => Some(err.clone()),
            _ => None,
        }
    }

    pub fn nanos_to_wait(&self) -> u64 {
        match self {
            Self::Wait(nanos) => *nanos,
            _ => 0,
        }
    }
}

impl fmt::Display for TokenResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenResult::Pass => write!(f, "TokenResult::Pass"),
            TokenResult::Blocked(block_err) => write!(f, "TokenResult::Blocked: {:?}", block_err),
            TokenResult::Wait(nanos_to_wait) => {
                write!(f, "TokenResult::Wait: {} ns", nanos_to_wait)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn register_block_new_type() {
        registry_block_type(BlockType::Other(100), "New").unwrap();
    }

    #[test]
    #[should_panic(expected = "Block type existed!")]
    fn register_block_exist_type() {
        registry_block_type(BlockType::HotSpotParamFlow, "BlockTypeHotSpotParamFlow").unwrap();
    }

    #[test]
    #[should_panic(expected = "Block type existed!")]
    fn register_block_new_type_twice() {
        registry_block_type(BlockType::Other(200), "New").unwrap();
        registry_block_type(BlockType::Other(200), "New").unwrap();
    }
}
