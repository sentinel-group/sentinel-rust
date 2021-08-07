use super::{EntryContext, ResourceWrapper, SlotChain};
use crate::logging;
use crate::{Error, Result};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::vec::Vec;

type ExitHandler = fn(entry: &SentinelEntry, ctx: Rc<RefCell<EntryContext>>) -> Result<()>;

pub struct SentinelEntry {
    /// inner context may need mutability in ExitHandlers, thus, RefCell is used
    ctx: Rc<RefCell<EntryContext>>,
    exit_handlers: Vec<ExitHandler>,
    /// each entry traverses a slot chain,
    /// global slot chain is wrapped by Arc, thus here we use Arc
    sc: Arc<SlotChain>,
}

impl SentinelEntry {
    pub fn new(ctx: Rc<RefCell<EntryContext>>, sc: Arc<SlotChain>) -> Self {
        SentinelEntry {
            ctx,
            exit_handlers: Vec::new(),
            sc,
        }
    }

    pub fn when_exit(&mut self, exit_handler: ExitHandler) {
        self.exit_handlers.push(exit_handler);
    }

    pub fn context(&self) -> Rc<RefCell<EntryContext>> {
        self.ctx.clone()
    }

    // todo: cleanup
    pub fn exit(&self) {
        for handler in &self.exit_handlers {
            handler(&self, self.ctx.clone())
                .map_err(|err: Error| {
                    logging::error!("ERROR: {}", err);
                })
                .unwrap();
        }
        self.sc.exit(self.ctx.clone());
    }
}

#[cfg(test)]
mod test {
    use super::*;
    static mut EXIT_FLAG: u8 = 0;
    fn exit_handler_mock(_entry: &SentinelEntry, _ctx: Rc<RefCell<EntryContext>>) -> Result<()> {
        unsafe {
            EXIT_FLAG += 1;
        }
        Ok(())
    }

    #[test]
    fn exit() {
        unsafe {
            EXIT_FLAG = 0;
        }
        let sc = Arc::new(SlotChain::new());
        let ctx = Rc::new(RefCell::new(EntryContext::new()));
        let mut entry = SentinelEntry::new(ctx.clone(), sc);

        entry.when_exit(exit_handler_mock);
        let entry = Rc::new(entry);
        ctx.borrow_mut().set_entry(Rc::downgrade(&entry));
        entry.exit();
        unsafe {
            assert_eq!(EXIT_FLAG, 1);
        }
    }
}
