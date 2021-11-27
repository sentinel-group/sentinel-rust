use lazy_static::lazy_static;
use time::{macros::format_description, Duration, OffsetDateTime};

lazy_static! {
    static ref UNIX_TIME_UNIT_OFFSET: i128 = (Duration::MILLISECOND / Duration::NANOSECOND) as i128;
}

#[inline]
pub fn unix_time_unit_offset() -> u64 {
    *UNIX_TIME_UNIT_OFFSET as u64
}

#[inline]
pub fn sleep_for_ms(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

#[inline]
pub fn sleep_for_ns(ns: u64) {
    std::thread::sleep(std::time::Duration::from_nanos(ns));
}

#[inline]
fn cal_curr_time_millis() -> u64 {
    (OffsetDateTime::now_utc().unix_timestamp_nanos() / (*UNIX_TIME_UNIT_OFFSET)) as u64
}

#[inline]
pub fn format_time_millis(ts_millis: u64) -> String {
    OffsetDateTime::from_unix_timestamp_nanos(milli2nano(ts_millis))
        .unwrap()
        .format(format_description!("[hour]:[minute]:[second]"))
        .unwrap()
}

#[inline]
/// The format is corresponding to `crate::log::metric::METRIC_FILE_PATTERN`
pub fn format_date(ts_millis: u64) -> String {
    OffsetDateTime::from_unix_timestamp_nanos(milli2nano(ts_millis))
        .unwrap()
        .format(format_description!("[year]-[month]-[day]"))
        .unwrap()
}

#[inline]
pub fn format_time_nanos_curr() -> String {
    OffsetDateTime::from_unix_timestamp_nanos(curr_time_nanos())
        .unwrap()
        .format(format_description!("[hour]:[minute]:[second]"))
        .unwrap()
}

pub fn curr_time_millis() -> u64 {
    // todo: conditional compilation, `config::use_cache_time()`
    let ticker_time = curr_time_millis_with_ticker();
    if ticker_time > 0 {
        ticker_time
    } else {
        cal_curr_time_millis()
    }
}

#[inline]
pub fn curr_time_nanos() -> i128 {
    OffsetDateTime::now_utc().unix_timestamp_nanos()
}

#[inline]
pub fn milli2nano<T: Into<i128>>(t: T) -> i128 {
    *UNIX_TIME_UNIT_OFFSET * t.into()
}

pub use ticker::*;

// provide cached time by a ticker
pub mod ticker {
    use super::*;
    use lazy_static::lazy_static;
    use std::sync::atomic::{AtomicU64, Ordering};

    lazy_static! {
        static ref NOW_IN_MS: AtomicU64 = AtomicU64::new(0);
    }

    /// `start_time_ticker()` starts a background task that caches current timestamp per millisecond,
    /// which may provide better performance in high-concurrency scenarios.
    pub fn start_time_ticker() {
        update_time();
        std::thread::spawn(move || loop {
            update_time();
            std::thread::sleep(std::time::Duration::from_millis(1));
        });
    }
    #[inline]
    fn update_time() {
        let curr = cal_curr_time_millis();
        NOW_IN_MS.store(curr, Ordering::SeqCst);
    }

    #[inline]
    pub(super) fn curr_time_millis_with_ticker() -> u64 {
        NOW_IN_MS.load(Ordering::SeqCst)
    }
}
