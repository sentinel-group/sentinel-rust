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
            if BLOCK_TYPE_MAP.lock().unwrap().contains_key(&id) {
                Err(Error::msg(EXIST_BLOCK_ERROR))
            } else {
                BLOCK_TYPE_MAP.lock().unwrap().insert(id, desc);
                Ok(())
            }
        }
        _ => Err(Error::msg(EXIST_BLOCK_ERROR)),
    }
}

impl fmt::Display for BlockType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let BlockType::Other(id) = self {
            match BLOCK_TYPE_MAP.lock().unwrap().get(id) {
                Some(&desc) => return write!(f, "{}", desc),
                None => return write!(f, "{}", id),
            }
        } else {
            return write!(f, "{:?}", self);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResultStatus {
    Pass,
    Blocked,
    ShouldWait,
}

impl Default for ResultStatus {
    fn default() -> Self {
        ResultStatus::Pass
    }
}

impl fmt::Display for ResultStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TokenResult {
    status: ResultStatus,
    block_err: Option<BlockError>,
    nanos_to_wait: u64,
}

impl TokenResult {
    pub fn new_pass() -> Self {
        Self::default()
    }

    pub fn new_should_wait(nanos_to_wait: u64) -> Self {
        Self {
            status: ResultStatus::ShouldWait,
            // here u64->i64 should not overflow, since it represents the waiting duration
            nanos_to_wait,
            ..Self::default()
        }
    }

    pub fn new_blocked(block_type: BlockType) -> Self {
        Self {
            status: ResultStatus::Blocked,
            block_err: Some(BlockError::new(block_type)),
            ..Self::default()
        }
    }

    pub fn new_blocked_with_msg(block_type: BlockType, block_msg: String) -> Self {
        Self {
            status: ResultStatus::Blocked,
            block_err: Some(BlockError::new_with_msg(block_type, block_msg)),
            ..Self::default()
        }
    }

    pub fn new_blocked_with_cause(
        block_type: BlockType,
        block_msg: String,
        rule: Arc<dyn SentinelRule>,
        snapshot_value: Arc<Snapshot>,
    ) -> Self {
        Self {
            status: ResultStatus::Blocked,
            block_err: Some(BlockError::new_with_cause(
                block_type,
                block_msg,
                rule,
                snapshot_value,
            )),
            ..Self::default()
        }
    }

    //[attention] think what would happen outside this function? add lifetime automatically? or &self would be dangling?
    pub fn reset_to_pass(&mut self) {
        self.status = ResultStatus::Pass;
        self.block_err = None;
        self.nanos_to_wait = 0;
    }

    pub fn reset_to_blocked(&mut self, block_type: BlockType) {
        self.status = ResultStatus::Blocked;
        self.block_err = Some(BlockError::new(block_type));
        self.nanos_to_wait = 0;
    }

    pub fn reset_to_blocked_with_msg(&mut self, block_type: BlockType, block_msg: String) {
        self.status = ResultStatus::Blocked;
        self.block_err = Some(BlockError::new_with_msg(block_type, block_msg));
        self.nanos_to_wait = 0;
    }

    pub fn reset_to_blocked_with_cause(
        &mut self,
        block_type: BlockType,
        block_msg: String,
        rule: Arc<dyn SentinelRule>,
        snapshot_value: Arc<Snapshot>,
    ) {
        self.status = ResultStatus::Blocked;
        self.block_err = Some(BlockError::new_with_cause(
            block_type,
            block_msg,
            rule,
            snapshot_value,
        ));
        self.nanos_to_wait = 0;
    }

    pub fn is_pass(&self) -> bool {
        self.status == ResultStatus::Pass
    }

    pub fn is_blocked(&self) -> bool {
        self.status == ResultStatus::Blocked
    }

    pub fn is_wait(&self) -> bool {
        self.status == ResultStatus::ShouldWait
    }

    pub fn status(&self) -> &ResultStatus {
        &self.status
    }
    pub fn block_err(&self) -> Option<BlockError> {
        self.block_err.clone()
    }
    pub fn nanos_to_wait(&self) -> u64 {
        self.nanos_to_wait
    }
}

impl fmt::Display for TokenResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.block_err.is_some() {
            write!(
                f,
                "TokenResult{{status={}, blockErr={:?}, nanosToWait={:?}}}",
                self.status,
                self.block_err.as_ref().unwrap(),
                self.nanos_to_wait
            )
        } else {
            write!(
                f,
                "TokenResult{{status={}, blockErr=None, nanosToWait={:?}}}",
                self.status, self.nanos_to_wait
            )
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
