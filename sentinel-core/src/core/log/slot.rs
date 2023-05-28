use crate::base::{BaseSlot, BlockError, EntryContext, StatSlot};
use lazy_static::lazy_static;
use std::sync::Arc;

const STAT_SLOT_ORDER: u32 = 2000;

lazy_static! {
    pub static ref DEFAULT_STAT_SLOT: Arc<Slot> = Arc::new(Slot {});
}

pub fn default_stat_slot() -> Arc<Slot> {
    DEFAULT_STAT_SLOT.clone()
}

pub struct Slot {}

impl BaseSlot for Slot {
    fn order(&self) -> u32 {
        STAT_SLOT_ORDER
    }
}

impl StatSlot for Slot {
    fn on_entry_pass(&self, _ctx: &EntryContext) {}

    // todo: write sentinel-block.log here
    fn on_entry_blocked(&self, _ctx: &EntryContext, _block_error: BlockError) {}

    fn on_completed(&self, _ctx: &mut EntryContext) {}
}
