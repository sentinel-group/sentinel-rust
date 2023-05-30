use super::ResourceNode;
use crate::{
    base::{ResourceType, DEFAULT_MAX_RESOURCE_AMOUNT, TOTAL_IN_BOUND_RESOURCE_NAME},
    logging,
};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

type ResourceNodeMap = HashMap<String, Arc<ResourceNode>>;

lazy_static! {
    pub static ref INBOUND_NODE: Arc<ResourceNode> = Arc::new(ResourceNode::new(
        TOTAL_IN_BOUND_RESOURCE_NAME.into(),
        ResourceType::Common
    ));
    static ref RESOURCE_NODE_MAP: RwLock<ResourceNodeMap> = RwLock::new(ResourceNodeMap::new());
}

pub fn inbound_node() -> Arc<ResourceNode> {
    INBOUND_NODE.clone()
}

// resource_node_list returns the slice of all existing resource nodes.
pub fn resource_node_list() -> Vec<Arc<ResourceNode>> {
    let res_map = RESOURCE_NODE_MAP.read().unwrap();
    res_map.values().cloned().collect()
}

pub fn get_resource_node(res_name: &String) -> Option<Arc<ResourceNode>> {
    let res_map = RESOURCE_NODE_MAP.read().unwrap();
    res_map.get(res_name).cloned()
}

pub fn get_or_create_resource_node(
    res_name: &String,
    resource_type: &ResourceType,
) -> Arc<ResourceNode> {
    let node = get_resource_node(res_name);
    match node {
        Some(node) => node,
        None => {
            if RESOURCE_NODE_MAP.read().unwrap().len() >= DEFAULT_MAX_RESOURCE_AMOUNT {
                logging::warn!(
                    "[get_or_create_resource_node] Resource amount exceeds the threshold {}",
                    DEFAULT_MAX_RESOURCE_AMOUNT
                )
            }
            RESOURCE_NODE_MAP.write().unwrap().insert(
                res_name.clone(),
                Arc::new(ResourceNode::new(res_name.clone(), *resource_type)),
            );
            RESOURCE_NODE_MAP
                .read()
                .unwrap()
                .get(res_name)
                .unwrap()
                .clone()
        }
    }
}

pub fn reset_resource_map() {
    RESOURCE_NODE_MAP.write().unwrap().clear();
}
