use super::*;
use crate::core::{circuitbreaker, flow, hotspot, isolation, system};

/// flow_rule_updater load the flow::Rule vector to downstream flow component.
fn flow_rule_updater(rules: Vec<Arc<flow::Rule>>) -> Result<bool> {
    Ok(flow::load_rules(rules))
}

pub fn new_flow_rule_handler(
    converter: PropertyConverter<flow::Rule>,
) -> Arc<impl PropertyHandler<flow::Rule>> {
    DefaultPropertyHandler::new(converter, flow_rule_updater)
}

/// system_rule_updater load the system::Rule vector to downstream flow component.
fn system_rule_updater(rules: Vec<Arc<system::Rule>>) -> Result<bool> {
    system::load_rules(rules);
    Ok(true)
}

pub fn new_system_rule_handler(
    converter: PropertyConverter<system::Rule>,
) -> Arc<impl PropertyHandler<system::Rule>> {
    DefaultPropertyHandler::new(converter, system_rule_updater)
}

/// circuitbreaker_rule_updater load the circuitbreaker::Rule vector to downstream flow component.
fn circuitbreaker_rule_updater(rules: Vec<Arc<circuitbreaker::Rule>>) -> Result<bool> {
    Ok(circuitbreaker::load_rules(rules))
}

pub fn new_circuitbreaker_rule_handler(
    converter: PropertyConverter<circuitbreaker::Rule>,
) -> Arc<impl PropertyHandler<circuitbreaker::Rule>> {
    DefaultPropertyHandler::new(converter, circuitbreaker_rule_updater)
}

/// isolation_rule_updater load the isolation::Rule vector to downstream flow component.
fn isolation_rule_updater(rules: Vec<Arc<isolation::Rule>>) -> Result<bool> {
    isolation::load_rules(rules);
    Ok(true)
}

pub fn new_isolation_rule_handler(
    converter: PropertyConverter<isolation::Rule>,
) -> Arc<impl PropertyHandler<isolation::Rule>> {
    DefaultPropertyHandler::new(converter, isolation_rule_updater)
}

/// hotspot_rule_updater load the hotspot::Rule vector to downstream flow component.
fn hotspot_rule_updater(rules: Vec<Arc<hotspot::Rule>>) -> Result<bool> {
    Ok(hotspot::load_rules(rules))
}

pub fn new_hotspot_rule_handler(
    converter: PropertyConverter<hotspot::Rule>,
) -> Arc<impl PropertyHandler<hotspot::Rule>> {
    DefaultPropertyHandler::new(converter, hotspot_rule_updater)
}
