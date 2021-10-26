use crate::Result;
use serde::Deserialize;
use std::fmt;
use std::hash::Hash;

pub trait SentinelRule: fmt::Debug + Send + Sync {
    fn resource_name(&self) -> String;
    fn is_valid(&self) -> Result<()> {
        Ok(())
    }
}
