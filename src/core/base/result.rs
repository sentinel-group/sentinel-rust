//! Result
//!
use super::{BlockError, SentinelRule};
use crate::{Error, Result};
use lazy_static::lazy_static;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::sync::Mutex;
use time::Duration;

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
pub enum TokenResultStatus {
    Pass,
    Blocked,
    ShouldWait,
}

impl Default for TokenResultStatus {
    fn default() -> Self {
        TokenResultStatus::Pass
    }
}

impl fmt::Display for TokenResultStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TokenResult {
    pub(crate) status: TokenResultStatus,
    pub(crate) block_err: Option<BlockError>,
    pub(crate) nanos_to_wait: Duration,
}

impl TokenResult {
    pub fn new_pass() -> Self {
        Self::default()
    }

    pub fn new_should_wait(wait_ns: Duration) -> Self {
        Self {
            status: TokenResultStatus::ShouldWait,
            nanos_to_wait: wait_ns,
            ..Self::default()
        }
    }

    pub fn new_blocked(block_type: BlockType) -> Self {
        Self {
            status: TokenResultStatus::Blocked,
            block_err: Some(BlockError::new(block_type)),
            ..Self::default()
        }
    }

    pub fn new_blocked_with_msg(block_type: BlockType, block_msg: String) -> Self {
        Self {
            status: TokenResultStatus::Blocked,
            block_err: Some(BlockError::new_with_msg(block_type, block_msg)),
            ..Self::default()
        }
    }

    pub fn new_blocked_with_cause(
        block_type: BlockType,
        block_msg: String,
        rule: Rc<RefCell<dyn SentinelRule>>,
        snapshot_value: Rc<RefCell<dyn Any>>,
    ) -> Self {
        Self {
            status: TokenResultStatus::Blocked,
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
        self.status = TokenResultStatus::Pass;
        self.block_err = None;
        self.nanos_to_wait = Duration::default();
    }

    pub fn reset_to_blocked(&mut self, block_type: BlockType) {
        self.status = TokenResultStatus::Blocked;
        self.block_err = Some(BlockError::new(block_type));
        self.nanos_to_wait = Duration::default();
    }

    pub fn reset_to_blocked_with_msg(&mut self, block_type: BlockType, block_msg: String) {
        self.status = TokenResultStatus::Blocked;
        self.block_err = Some(BlockError::new_with_msg(block_type, block_msg));
        self.nanos_to_wait = Duration::default();
    }

    pub fn reset_to_blocked_with_cause(
        &mut self,
        block_type: BlockType,
        block_msg: String,
        rule: Rc<RefCell<dyn SentinelRule>>,
        snapshot_value: Rc<RefCell<dyn Any>>,
    ) {
        self.status = TokenResultStatus::Blocked;
        self.block_err = Some(BlockError::new_with_cause(
            block_type,
            block_msg,
            rule,
            snapshot_value,
        ));
        self.nanos_to_wait = Duration::default();
    }

    pub fn is_pass(&self) -> bool {
        self.status == TokenResultStatus::Pass
    }

    pub fn is_blocked(&self) -> bool {
        self.status == TokenResultStatus::Blocked
    }

    pub fn status(&self) -> &TokenResultStatus {
        &self.status
    }
    pub fn block_err(&self) -> Option<&BlockError> {
        self.block_err.as_ref()
    }
    pub fn nanos_to_wait(&self) -> &Duration {
        &self.nanos_to_wait
    }
}

impl fmt::Display for TokenResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.block_err.is_some() {
            write!(
                f,
                "TokenResult{{status={}, blockErr={}, nanosToWait={:?}}}",
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
