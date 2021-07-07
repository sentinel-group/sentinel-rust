use super::{BlockType, SentinelRule};
use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

// BlockError indicates the request was blocked by Sentinel.
#[derive(Debug, Clone, Default)]
pub struct BlockError {
    pub(crate) block_type: BlockType,
    // blockMsg provides additional message for the block error.
    pub(crate) block_msg: String,
    pub(crate) rule: Option<Rc<RefCell<dyn SentinelRule>>>,
    // snapshotValue represents the triggered "snapshot" value
    pub(crate) snapshot_value: Option<Rc<RefCell<dyn Any>>>,
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
        rule: Rc<RefCell<dyn SentinelRule>>,
        snapshot_value: Rc<RefCell<dyn Any>>,
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

    pub fn triggered_rule(&self) -> Option<Rc<RefCell<dyn SentinelRule>>> {
        self.rule.clone()
    }

    pub fn triggered_value(&self) -> Option<Rc<RefCell<dyn Any>>> {
        self.snapshot_value.clone()
    }
}

impl fmt::Display for BlockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.block_msg.len() == 0 {
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
    use super::*;
    #[derive(Debug, Default)]
    struct MockRule {
        id: String,
    }

    impl SentinelRule for MockRule {
        fn resource_name(&self) -> String {
            return "mock resource".into();
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
        rule: Option<Rc<RefCell<dyn SentinelRule>>>,
        snapshot_value: Option<Rc<RefCell<dyn Any>>>,
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
            assert!(Rc::ptr_eq(&block_err.triggered_rule().unwrap(), &rule));
            assert!(Rc::ptr_eq(
                &block_err.triggered_value().unwrap(),
                &snapshot_value
            ));
        } else {
            if let Some(block_msg) = block_msg {
                block_err = BlockError::new_with_msg(block_type, block_msg.clone());
                assert_eq!(block_err.block_type, block_type);
                assert_eq!(block_err.block_msg, block_msg);
                assert_eq!(block_err.triggered_rule().is_none(), true);
                assert_eq!(block_err.triggered_value().is_none(), true);
            } else {
                block_err = BlockError::new(block_type);
                assert_eq!(block_err.block_type, block_type);
                assert_eq!(block_err.block_msg, String::default());
                assert_eq!(block_err.triggered_rule().is_none(), true);
                assert_eq!(block_err.triggered_value().is_none(), true);
            }
        }
    }

    #[test]
    fn error_create() {
        testcase(BlockType::Flow, None, None, None);
        testcase(BlockType::Flow, Some(String::from("mock msg")), None, None);
        testcase(
            BlockType::Flow,
            Some(String::from("mock msg")),
            Some(Rc::new(RefCell::new(MockRule::default()))),
            Some(Rc::new(RefCell::new(String::from("mock value")))),
        );
    }
}
