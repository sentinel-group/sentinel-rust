//! Resource/Traffic Wrappers
use std::fmt;

/// ResourceType represents classification of the resources
#[derive(Debug, Copy, Clone)]
pub enum ResourceType {
    Common,
    Web,
    RPC,
    APIGateway,
    DBSQL,
    Cache,
    MQ,
}

/// TrafficType describes the traffic type: Inbound or Outbound
#[derive(Debug, Copy, Clone)]
pub enum TrafficType {
    Inbound,
    Outbound,
}

/// ResourceWrapper represents the invocation
#[derive(Debug, Clone)]
pub struct ResourceWrapper {
    /// global unique resource name
    name: String,
    /// resource classification
    classification: ResourceType,
    /// Inbound or Outbound
    flow_type: TrafficType,
}

impl fmt::Display for ResourceWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ResourceWrapper{{name={}, flowType={:?}, classification={:?}}}",
            self.name, self.flow_type, self.classification
        )
    }
}

impl ResourceWrapper {
    pub fn new(name: String, classification: ResourceType, flow_type: TrafficType) -> Self {
        Self {
            name,
            classification,
            flow_type,
        }
    }
    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn classification(&self) -> ResourceType {
        self.classification.clone()
    }
    pub fn flow_type(&self) -> TrafficType {
        self.flow_type.clone()
    }
}
