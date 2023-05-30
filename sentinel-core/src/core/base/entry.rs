use super::{ContextPtr, SlotChain};
use crate::logging;
use crate::{Error, Result};
use std::sync::Arc;
use std::sync::{RwLock, Weak};
use std::vec::Vec;

type ExitHandler = Box<dyn Send + Sync + Fn(&SentinelEntry, ContextPtr) -> Result<()>>;

// currently, ctx and entry are N:M mapped,
// and they may be used in async contexts,
// therefore, we need Arc (for Sync and Send) and RwLock (for inner mutability)
type EntryStrongPtrInner = Arc<RwLock<SentinelEntry>>;
pub struct EntryStrongPtr(EntryStrongPtrInner);
pub type EntryWeakPtr = Weak<RwLock<SentinelEntry>>;

pub struct SentinelEntry {
    ctx: ContextPtr,
    exit_handlers: Vec<ExitHandler>,
    /// each entry traverses a slot chain,
    /// global slot chain is wrapped by Arc, thus here we use Arc
    sc: Arc<SlotChain>,
}

impl SentinelEntry {
    pub fn new(ctx: ContextPtr, sc: Arc<SlotChain>) -> Self {
        SentinelEntry {
            ctx,
            exit_handlers: Vec::new(),
            sc,
        }
    }

    pub fn when_exit(&mut self, exit_handler: ExitHandler) {
        self.exit_handlers.push(exit_handler);
    }

    pub fn context(&self) -> &ContextPtr {
        &self.ctx
    }

    pub fn set_err(&self, err: Error) {
        self.ctx.write().unwrap().set_err(err);
    }

    // todo: cleanup
    pub fn exit(&self) {
        for handler in &self.exit_handlers {
            handler(self, self.ctx.clone()) // Rc/Arc clone
                .map_err(|err: Error| {
                    logging::error!("ERROR: {}", err);
                })
                .unwrap();
        }
        self.sc.exit(self.ctx.clone()); // Rc/Arc clone
    }
}

impl EntryStrongPtr {
    pub fn new(entry: EntryStrongPtrInner) -> EntryStrongPtr {
        EntryStrongPtr(entry)
    }

    pub fn context(&self) -> ContextPtr {
        let entry = self.0.read().unwrap();
        entry.context().clone()
    }

    pub fn set_err(&self, err: Error) {
        self.0.read().unwrap().set_err(err);
    }

    pub fn exit(&self) {
        self.0.read().unwrap().exit();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::base::EntryContext;
    use std::cell::RefCell;
    use std::sync::RwLock;

    std::thread_local! {
        static EXIT_FLAG: RefCell<u8> = RefCell::new(0);
    }
    fn exit_handler_mock(_entry: &SentinelEntry, _ctx: Arc<RwLock<EntryContext>>) -> Result<()> {
        EXIT_FLAG.with(|f| {
            *f.borrow_mut() += 1;
        });
        Ok(())
    }

    #[test]
    fn exit() {
        let sc = Arc::new(SlotChain::new());
        let ctx = Arc::new(RwLock::new(EntryContext::new()));
        let mut entry = SentinelEntry::new(ctx.clone(), sc);

        entry.when_exit(Box::new(exit_handler_mock));
        let entry = Arc::new(RwLock::new(entry));
        ctx.write().unwrap().set_entry(Arc::downgrade(&entry));
        entry.read().unwrap().exit();
        EXIT_FLAG.with(|f| {
            assert_eq!(*f.borrow(), 1);
        });
    }
}
