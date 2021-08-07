//! Resource/Traffic Wrappers
use crate::utils::format_time_nanos_curr;
use serde::{Deserialize, Serialize};
use std::fmt;

/// ResourceType represents resource_type of the resources
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ResourceType {
    Common = 0,
    Web,
    RPC,
    APIGateway,
    DBSQL,
    Cache,
    MQ,
}

impl Default for ResourceType {
    fn default() -> ResourceType {
        ResourceType::Common
    }
}

// or use `num_enum` crate
impl From<u8> for ResourceType {
    fn from(num: u8) -> ResourceType {
        match num {
            1 => ResourceType::Web,
            2 => ResourceType::RPC,
            3 => ResourceType::APIGateway,
            4 => ResourceType::DBSQL,
            5 => ResourceType::Cache,
            6 => ResourceType::MQ,
            _ => ResourceType::Common,
        }
    }
}

/// TrafficType describes the traffic type: Inbound or Outbound
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TrafficType {
    Outbound,
    Inbound,
}

impl Default for TrafficType {
    fn default() -> TrafficType {
        TrafficType::Outbound
    }
}

/// ResourceWrapper represents the invocation
#[derive(Debug, Clone)]
pub struct ResourceWrapper {
    /// global unique resource name
    name: String,
    /// resource resource_type
    resource_type: ResourceType,
    /// Inbound or Outbound
    flow_type: TrafficType,
}

impl Default for ResourceWrapper {
    fn default() -> Self {
        ResourceWrapper {
            name: format_time_nanos_curr(),
            resource_type: ResourceType::default(),
            flow_type: TrafficType::default(),
        }
    }
}

impl fmt::Display for ResourceWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ResourceWrapper{{name={}, flowType={:?}, resource_type={:?}}}",
            self.name, self.flow_type, self.resource_type
        )
    }
}

impl ResourceWrapper {
    pub fn new(name: String, resource_type: ResourceType, flow_type: TrafficType) -> Self {
        ResourceWrapper {
            name,
            resource_type,
            flow_type,
        }
    }
    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn resource_type(&self) -> &ResourceType {
        &self.resource_type
    }
    pub fn flow_type(&self) -> &TrafficType {
        &self.flow_type
    }
}
