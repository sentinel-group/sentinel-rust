use super::{EntryContext, ResourceWrapper, SlotChain};
use crate::logging::log::error;
use crate::{Error, Result};
use std::cell::RefCell;
use std::rc::{Weak, Rc};
use std::vec::Vec;

type ExitHandler = fn(entry: &SentinelEntry, ctx: Rc<RefCell<EntryContext>>) -> Result<()>;

pub struct SentinelEntry {
    res: Option<ResourceWrapper>,
    /// one entry bounds with one context
    ctx: Weak<RefCell<EntryContext>>,
    exit_handlers: Vec<ExitHandler>,
    /// each entry holds a slot chain.
    /// it means this entry will go through the sc
    sc: Rc<RefCell<SlotChain>>,
}

impl SentinelEntry {
    pub fn new(
        res: Option<ResourceWrapper>,
        ctx: Weak<RefCell<EntryContext>>,
        sc: Rc<RefCell<SlotChain>>,
    ) -> Self {
        Self {
            res,
            ctx,
            exit_handlers: Vec::new(),
            sc,
        }
    }

    pub fn when_exit(&mut self, exit_handler: ExitHandler) {
        self.exit_handlers.push(exit_handler);
    }

    pub fn context(&self) -> Weak<RefCell<EntryContext>> {
        self.ctx.clone()
    }

    pub fn resource(&self) -> Option<&ResourceWrapper> {
        self.res.as_ref()
    }

    // todo: cleanup
    pub fn exit(self) {
        for handler in &self.exit_handlers {
            handler(&self, self.ctx.upgrade().unwrap())
                .map_err(|err: Error| {
                    error!("ERROR: {}", err);
                })
                .unwrap();
        }
        self.sc.borrow().exit(self.ctx.upgrade().unwrap());
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
        let sc = Rc::new(RefCell::new(SlotChain::new()));
        let ctx = Rc::new(RefCell::new(EntryContext::new()));
        let mut entry =
            SentinelEntry::new(None, Rc::downgrade(&ctx), sc);
        entry.when_exit(exit_handler_mock);
        entry.exit();
        unsafe {
            assert_eq!(EXIT_FLAG, 1);
        }
    }
}
