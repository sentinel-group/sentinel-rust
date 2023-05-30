//! MemoryAdaptiveCalculator is a memory adaptive traffic shaping calculator
//!
//! Adaptive flow control algorithm, threshold changes with water mark linearly
//! That is, the mapping is:
//! - If the water_mark is less than Rule.mem_low_water_mark, the threshold is Rule.low_mem_usage_threshold.
//! - If the water_mark is greater than Rule.mem_high_water_mark, the threshold is Rule.high_mem_usage_threshold.
//! - Otherwise, the threshold is `((water_mark - mem_low_water_mark)/(mem_high_water_mark - mem_low_water_mark)) *
//! (high_mem_usage_threshold - low_mem_usage_threshold) + low_mem_usage_threshold`.
//!

use super::Rule;
use super::{Calculator, Controller};
use crate::system_metric;
use std::sync::{Arc, Weak};

#[derive(Debug)]
pub struct MemoryAdaptiveCalculator {
    owner: Weak<Controller>,
    mem_low_water_mark: f64,
    mem_high_water_mark: f64,
    low_mem_usage_threshold: f64,
    high_mem_usage_threshold: f64,
}

impl MemoryAdaptiveCalculator {
    pub fn new(owner: Weak<Controller>, rule: Arc<Rule>) -> Self {
        MemoryAdaptiveCalculator {
            owner,
            mem_low_water_mark: rule.mem_low_water_mark as f64,
            mem_high_water_mark: rule.mem_high_water_mark as f64,
            low_mem_usage_threshold: rule.low_mem_usage_threshold as f64,
            high_mem_usage_threshold: rule.high_mem_usage_threshold as f64,
        }
    }
}

impl Calculator for MemoryAdaptiveCalculator {
    fn get_owner(&self) -> &Weak<Controller> {
        &self.owner
    }

    fn set_owner(&mut self, owner: Weak<Controller>) {
        self.owner = owner;
    }

    fn calculate_allowed_threshold(&self, _batch_count: u32, _flag: i32) -> f64 {
        let mem = system_metric::current_memory_usage() as f64;
        if mem > self.mem_high_water_mark {
            self.high_mem_usage_threshold
        } else if mem < self.mem_low_water_mark {
            self.low_mem_usage_threshold
        } else {
            // linear mapping
            (self.high_mem_usage_threshold - self.low_mem_usage_threshold)
                / (self.mem_high_water_mark - self.mem_low_water_mark)
                * (mem - self.mem_low_water_mark)
                + self.low_mem_usage_threshold
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn calculator() {
        let tc = MemoryAdaptiveCalculator {
            owner: Weak::new(),
            mem_low_water_mark: 1024.0,
            mem_high_water_mark: 2048.0,
            low_mem_usage_threshold: 1000.0,
            high_mem_usage_threshold: 100.0,
        };
        system_metric::set_memory_usage(100);
        assert!(
            (tc.calculate_allowed_threshold(0, 0) - tc.low_mem_usage_threshold).abs()
                < f64::EPSILON
        );
        system_metric::set_memory_usage(1024);
        assert!(
            (tc.calculate_allowed_threshold(0, 0) - tc.low_mem_usage_threshold).abs()
                < f64::EPSILON
        );
        system_metric::set_memory_usage(1536);
        assert!((tc.calculate_allowed_threshold(0, 0) - 550.0).abs() < f64::EPSILON);
        system_metric::set_memory_usage(2048);
        assert!((tc.calculate_allowed_threshold(0, 0) - 100.0).abs() < f64::EPSILON);
        system_metric::set_memory_usage(3072);
        assert!((tc.calculate_allowed_threshold(0, 0) - 100.0).abs() < f64::EPSILON);
    }
}
