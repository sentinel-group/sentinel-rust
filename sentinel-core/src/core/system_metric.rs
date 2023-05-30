use crate::{logging, utils, Result};
cfg_exporter! {
    use crate::exporter;
}
use lazy_static::lazy_static;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex, Once,
};
use sysinfo::{get_current_pid, ProcessExt, System, SystemExt};

lazy_static! {
    static ref SYSTEM: Arc<Mutex<System>> = Arc::new(Mutex::new(System::new_all()));
    static ref CURRENT_CPU: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));
    static ref CURRENT_MEMORY: AtomicU64 = AtomicU64::new(0);
    static ref CURRENT_LOAD: Arc<Mutex<f64>> = Arc::new(Mutex::new(0.0));
    static ref LOAD_ONCE: Once = Once::new();
    static ref CPU_ONCE: Once = Once::new();
    static ref MEMORY_ONCE: Once = Once::new();
}

/// get_total_memory_size returns the current machine's memory statistic in KBytes
pub fn get_total_memory_size() -> u64 {
    let mut system = SYSTEM.lock().unwrap();
    system.refresh_memory();
    system.total_memory()
}

pub fn init_memory_collector(mem_interval: u32) {
    if mem_interval == 0 {
        return;
    }
    MEMORY_ONCE.call_once(move || {
        std::thread::spawn(move || loop {
            let memory_used_bytes = get_process_memory_stat();
            #[cfg(feature = "exporter")]
            exporter::set_memory_size(memory_used_bytes);
            CURRENT_MEMORY.store(memory_used_bytes, Ordering::SeqCst);
            utils::sleep_for_ms(mem_interval as u64);
        });
    });
    // Windows needs more time to start the collector thread
    // and acquire the lock on SYSTEM
    #[cfg(windows)]
    utils::sleep_for_ms(4000);
}

#[inline]
/// get_process_memory_stat gets current process's memory usage in KBytes
fn get_process_memory_stat() -> u64 {
    let mut system = SYSTEM.lock().unwrap();
    match get_current_pid() {
        Ok(pid) => {
            system.refresh_process(pid);
            let process = system.process(pid).unwrap();
            process.memory()
        }
        Err(_) => 0,
    }
}

pub fn init_cpu_collector(cpu_interval: u32) {
    if cpu_interval == 0 {
        return;
    }
    CPU_ONCE.call_once(move || {
        std::thread::spawn(move || loop {
            let cpu_percent = get_process_cpu_stat();
            #[cfg(feature = "exporter")]
            exporter::set_cpu_ratio(cpu_percent);
            *CURRENT_CPU.lock().unwrap() = cpu_percent;
            utils::sleep_for_ms(cpu_interval as u64);
        });
    });
    // Windows needs more time to start the collector thread
    // and acquire the lock on SYSTEM
    #[cfg(windows)]
    utils::sleep_for_ms(4000);
}

#[inline]
fn get_process_cpu_stat() -> f32 {
    let mut system = SYSTEM.lock().unwrap();
    match get_current_pid() {
        Ok(pid) => {
            system.refresh_process(pid);
            let process = system.process(pid).unwrap();
            process.cpu_usage()
        }
        Err(_) => 0.0,
    }
}

pub fn init_load_collector(load_interval: u32) {
    if load_interval == 0 {
        return;
    }
    LOAD_ONCE.call_once(move || {
        std::thread::spawn(move || loop {
            let load = get_system_load().unwrap_or_else(|_| {
                logging::error!(
                    "[retrieveAndUpdateSystemStat] Failed to retrieve current system load"
                );
                0.0
            });
            *CURRENT_LOAD.lock().unwrap() = load;
            utils::sleep_for_ms(load_interval as u64);
        });
    });
    // Windows needs more time to start the collector thread
    // and acquire the lock on SYSTEM
    #[cfg(windows)]
    utils::sleep_for_ms(4000);
}

#[inline]
fn get_system_load() -> Result<f64> {
    let system = SYSTEM.lock().unwrap();
    let avg = system.load_average();
    Ok(avg.one)
}

#[inline]
pub fn current_load() -> f64 {
    *CURRENT_LOAD.lock().unwrap()
}

#[cfg(test)]
#[inline]
pub fn set_system_load(load: f64) {
    *CURRENT_LOAD.lock().unwrap() = load;
}

#[inline]
pub fn current_cpu_usage() -> f32 {
    *CURRENT_CPU.lock().unwrap()
}

#[cfg(test)]
#[inline]
pub fn set_cpu_usage(usage: f32) {
    *CURRENT_CPU.lock().unwrap() = usage;
}

#[inline]
pub fn current_memory_usage() -> u64 {
    CURRENT_MEMORY.load(Ordering::SeqCst)
}

#[cfg(test)]
#[inline]
pub fn set_memory_usage(usage: u64) {
    CURRENT_MEMORY.store(usage, Ordering::SeqCst)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::utils::curr_time_millis;

    #[test]
    #[ignore]
    fn system_load() {
        set_system_load(0.0);
        let load = current_load();
        assert!((0.0 - load).abs() < f64::EPSILON);
        set_system_load(1.0);
        let load = current_load();
        assert!((1.0 - load).abs() < f64::EPSILON);
    }

    #[test]
    #[ignore]
    fn cpu_usage() {
        set_cpu_usage(0.0);
        let usage = current_cpu_usage();
        assert!((0.0 - usage).abs() < f32::EPSILON);
        set_cpu_usage(0.3);
        let usage = current_cpu_usage();
        assert!((0.3 - usage).abs() < f32::EPSILON);
    }

    #[test]
    #[ignore]
    fn memory_usage() {
        let usage = current_memory_usage();
        assert_eq!(0, usage);
        set_memory_usage(200);
        let usage = current_memory_usage();
        assert_eq!(200, usage);
    }

    #[test]
    #[ignore]
    #[cfg(not(target_os = "macos"))]
    fn process_cpu_stat() {
        std::thread::spawn(|| loop {
            let start = curr_time_millis();
            while curr_time_millis() - start < 50 {
                let _ = 0;
            }
            utils::sleep_for_ms(20);
        });
        set_cpu_usage(0.0);
        assert!((current_cpu_usage() - 0.0).abs() < f32::EPSILON);
        init_cpu_collector(50);
        utils::sleep_for_ms(500);
        assert!(current_cpu_usage() > 0.0);
    }
}
