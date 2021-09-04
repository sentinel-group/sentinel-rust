use crate::{logging, utils, Error, Result};
cfg_monitor! {
    use crate::monitor;
}
use lazy_static::lazy_static;
use psutil::{host, memory, process::Process};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex, Once,
};
use std::time;

lazy_static! {
    static ref CURRENT_CPU: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));
    static ref CURRENT_MEMORY: AtomicU64 = AtomicU64::new(0);
    static ref CURRENT_LOAD: Arc<Mutex<f64>> = Arc::new(Mutex::new(0.0));
    static ref LOAD_ONCE: Once = Once::new();
    static ref CPU_ONCE: Once = Once::new();
    static ref MEMORY_ONCE: Once = Once::new();
    static ref CURRENT_PROCESS: Arc<Mutex<Process>> =
        Arc::new(Mutex::new(Process::new(std::process::id()).unwrap()));
    static ref TOTAL_MEMORY_SIZE: u64 = get_total_memory_size();
}

/// getMemoryStat returns the current machine's memory statistic
pub fn get_total_memory_size() -> u64 {
    let vm = memory::virtual_memory();
    if let Ok(vm) = vm {
        vm.total()
    } else {
        logging::error!("Fail to read Virtual Memory");
        0
    }
}

pub fn init_memory_collector(cpu_interval: u32) {
    if cpu_interval == 0 {
        return;
    }
    MEMORY_ONCE.call_once(move || {
        std::thread::spawn(move || {
            let memory_used_bytes = get_process_memory_stat().unwrap_or_else(|_| {
                logging::error!("Fail to retrieve and update cpu statistic");
                0
            });
            #[cfg(feature = "monitor")]
            monitor::set_process_memory_size(memory_used_bytes);
            CURRENT_MEMORY.store(memory_used_bytes, Ordering::SeqCst);
            utils::sleep_for_ms(cpu_interval as u64);
        });
    })
}

#[inline]
// get_process_memory_stat gets current process's memory usage in Bytes
fn get_process_memory_stat() -> Result<u64> {
    let process = CURRENT_PROCESS.lock().unwrap();
    process
        .memory_info()
        .map(|res| res.rss())
        .map_err(|err| Error::msg(format!("{:?}", err)))
}

pub fn init_cpu_collector(cpu_interval: u32) {
    if cpu_interval == 0 {
        return;
    }
    CPU_ONCE.call_once(move || {
        std::thread::spawn(move || {
            let mut cpu_percent = get_process_cpu_stat().unwrap_or_else(|_| {
                logging::error!("Fail to retrieve and update cpu statistic");
                0.0
            });
            #[cfg(feature = "monitor")]
            monitor::set_cpu_ratio(cpu_percent);
            *CURRENT_CPU.lock().unwrap() = cpu_percent;
            utils::sleep_for_ms(cpu_interval as u64);
        });
    })
}

#[inline]
fn get_process_cpu_stat() -> Result<f32> {
    let mut process = CURRENT_PROCESS.lock().unwrap();
    process
        .cpu_percent()
        .map_err(|err| Error::msg(format!("{:?}", err)))
}

pub fn init_load_collector(load_interval: u32) {
    if load_interval == 0 {
        return;
    }
    LOAD_ONCE.call_once(move || {
        std::thread::spawn(move || {
            let mut load = get_system_load().unwrap_or_else(|_| {
                logging::error!(
                    "[retrieveAndUpdateSystemStat] Failed to retrieve current system load"
                );
                0.0
            });
            *CURRENT_LOAD.lock().unwrap() = load;
            utils::sleep_for_ms(load_interval as u64);
        });
    })
}

#[inline]
fn get_system_load() -> Result<f64> {
    let avg = host::loadavg()?;
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

    #[test]
    #[ignore]
    fn system_load() {
        let load = current_load();
        assert_eq!(0.0, load);
        set_system_load(1.0);
        let load = current_load();
        assert_eq!(1.0, load);
    }

    #[test]
    #[ignore]
    fn cpu_usage() {
        set_cpu_usage(0.0);
        let usage = current_cpu_usage();
        assert_eq!(0.0, usage);
        set_cpu_usage(0.3);
        let usage = current_cpu_usage();
        assert_eq!(0.3, usage);
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
    fn process_cpu_stat() {
        std::thread::spawn(|| {
            let mut i = 0;
            loop {
                i += 1;
            }
        });

        let got = get_process_cpu_stat().unwrap();

        assert_eq!(got, 0.0);
        utils::sleep_for_ms(20);
        let got = get_process_cpu_stat().unwrap();

        assert!(got > 0.0);
        utils::sleep_for_ms(20);

        let got = get_process_cpu_stat().unwrap();
        assert!(got > 0.0);
    }
}
