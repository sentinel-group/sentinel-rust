use std::fmt;

pub trait SentinelRule: fmt::Debug + fmt::Display {
    fn resource_name(&self) -> String;
}
