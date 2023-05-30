use super::{BlockType, SentinelRule};
use crate::utils;
use std::any::Any;
use std::fmt;
use std::sync::Arc;

// todo: use String instead of Any to record snapshots?
pub trait SnapshotTrait: Any + fmt::Debug + utils::AsAny + Send + Sync {}
impl<T: Any + fmt::Debug + utils::AsAny + Send + Sync> SnapshotTrait for T {}
pub type Snapshot = dyn SnapshotTrait;

// BlockError indicates the request was blocked by Sentinel.
#[derive(Debug, Clone, Default)]
pub struct BlockError {
    block_type: BlockType,
    // blockMsg provides additional message for the block error.
    block_msg: String,
    rule: Option<Arc<dyn SentinelRule>>,
    // snapshotValue represents the triggered "snapshot" value
    snapshot_value: Option<Arc<Snapshot>>,
}

impl PartialEq for BlockError {
    fn eq(&self, other: &BlockError) -> bool {
        self.block_type == other.block_type && self.block_msg == other.block_msg
    }
}

impl BlockError {
    pub fn new(block_type: BlockType) -> Self {
        Self {
            block_type,
            ..Self::default()
        }
    }

    pub fn new_with_msg(block_type: BlockType, block_msg: String) -> Self {
        Self {
            block_type,
            block_msg,
            ..Self::default()
        }
    }

    pub fn new_with_cause(
        block_type: BlockType,
        block_msg: String,
        rule: Arc<dyn SentinelRule>,
        snapshot_value: Arc<Snapshot>,
    ) -> Self {
        Self {
            block_type,
            block_msg,
            rule: Some(rule),
            snapshot_value: Some(snapshot_value),
        }
    }

    pub fn block_type(&self) -> BlockType {
        self.block_type
    }

    pub fn block_msg(&self) -> String {
        self.block_msg.clone()
    }

    pub fn triggered_rule(&self) -> Option<Arc<dyn SentinelRule>> {
        self.rule.clone()
    }

    pub fn triggered_value(&self) -> Option<Arc<Snapshot>> {
        self.snapshot_value.clone()
    }
}

impl fmt::Display for BlockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.block_msg.is_empty() {
            write!(f, "SentinelBlockError: {}", self.block_type)
        } else {
            write!(
                f,
                "NoBlockError: {}, message: {}",
                self.block_type, self.block_msg
            )
        }
    }
}

#[cfg(test)]
mod test {
    #![allow(clippy::vtable_address_comparisons)]

    use super::*;
    #[derive(Debug, Default)]
    struct MockRule {}

    impl SentinelRule for MockRule {
        fn resource_name(&self) -> String {
            "mock resource".into()
        }
    }

    impl fmt::Display for MockRule {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "mock rule")
        }
    }

    fn testcase(
        block_type: BlockType,
        block_msg: Option<String>,
        rule: Option<Arc<dyn SentinelRule>>,
        snapshot_value: Option<Arc<Snapshot>>,
    ) {
        let block_err: BlockError;
        if let (Some(rule), Some(snapshot_value), Some(block_msg)) =
            (rule, snapshot_value, block_msg.clone())
        {
            block_err = BlockError::new_with_cause(
                block_type,
                block_msg.clone(),
                rule.clone(),
                snapshot_value.clone(),
            );
            assert_eq!(block_err.block_type(), block_type);
            assert_eq!(block_err.block_msg(), block_msg);
            assert!(Arc::ptr_eq(&block_err.triggered_rule().unwrap(), &rule));
            assert!(Arc::ptr_eq(
                &block_err.triggered_value().unwrap(),
                &snapshot_value
            ));
        } else if let Some(block_msg) = block_msg {
            block_err = BlockError::new_with_msg(block_type, block_msg.clone());
            assert_eq!(block_err.block_type, block_type);
            assert_eq!(block_err.block_msg, block_msg);
            assert!(block_err.triggered_rule().is_none());
            assert!(block_err.triggered_value().is_none());
        } else {
            block_err = BlockError::new(block_type);
            assert_eq!(block_err.block_type, block_type);
            assert_eq!(block_err.block_msg, String::default());
            assert!(block_err.triggered_rule().is_none());
            assert!(block_err.triggered_value().is_none());
        }
    }

    #[test]
    fn error_create() {
        testcase(BlockType::Flow, None, None, None);
        testcase(BlockType::Flow, Some(String::from("mock msg")), None, None);
        testcase(
            BlockType::Flow,
            Some(String::from("mock msg")),
            Some(Arc::new(MockRule::default())),
            Some(Arc::new(String::from("mock value"))),
        );
    }
}
