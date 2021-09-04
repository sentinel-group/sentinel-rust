use super::{ContextPtr, EntryContext, ResourceWrapper, SlotChain};
use crate::logging;
use crate::{Error, Result};
use std::sync::Arc;
use std::vec::Vec;

type ExitHandler = Box<dyn Send + Sync + Fn(&SentinelEntry, ContextPtr) -> Result<()>>;

cfg_async! {
    use std::sync::{RwLock, Weak};
    pub type EntryStrongPtr = Arc<RwLock<SentinelEntry>>;
    pub type EntryWeakPtr = Weak<RwLock<SentinelEntry>>;
}

cfg_not_async! {
    use std::rc::{Rc,Weak};
    use std::cell::RefCell;
    pub type EntryStrongPtr = Rc<RefCell<SentinelEntry>>;
    pub type EntryWeakPtr = Weak<RefCell<SentinelEntry>>;
}

pub struct SentinelEntry {
    // todo: it is assumed that entry and context is visited in a single thread,
    // is it neccessary to consider concurrency?
    // Then Rc and RefCell is not suitable...
    /// inner context may need mutability in ExitHandlers, thus, RefCell is used
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

    // todo: cleanup
    pub fn exit(&self) {
        for handler in &self.exit_handlers {
            handler(&self, self.ctx.clone()) // Rc/Arc clone
                .map_err(|err: Error| {
                    logging::error!("ERROR: {}", err);
                })
                .unwrap();
        }
        self.sc.exit(self.ctx.clone()); // Rc/Arc clone
    }
}

#[cfg(test)]
mod test {
    use super::*;
    std::thread_local! {
        static EXIT_FLAG: RefCell<u8> = RefCell::new(0);
    }
    fn exit_handler_mock(_entry: &SentinelEntry, _ctx: Rc<RefCell<EntryContext>>) -> Result<()> {
        EXIT_FLAG.with(|f| {
            *f.borrow_mut() += 1;
        });
        Ok(())
    }

    #[test]
    fn exit() {
        let sc = Arc::new(SlotChain::new());
        let ctx = Rc::new(RefCell::new(EntryContext::new()));
        let mut entry = SentinelEntry::new(ctx.clone(), sc);

        entry.when_exit(Box::new(exit_handler_mock));
        let entry = Rc::new(RefCell::new(entry));
        ctx.borrow_mut().set_entry(Rc::downgrade(&entry));
        entry.borrow().exit();
        EXIT_FLAG.with(|f| {
            assert_eq!(*f.borrow(), 1);
        });
    }
}
