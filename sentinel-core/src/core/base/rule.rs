use crate::Result;
use std::fmt;

pub trait SentinelRule: fmt::Debug + Send + Sync {
    fn resource_name(&self) -> String;
    fn is_valid(&self) -> Result<()> {
        Ok(())
    }
}
