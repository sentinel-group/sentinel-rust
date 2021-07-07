use lazy_static::lazy_static;
use time::{Duration, OffsetDateTime};

lazy_static! {
    static ref UNIX_TIME_UNIT_OFFSET: i128 =
        (Duration::millisecond() / Duration::nanosecond()) as i128;
}
const TIME_FORMAT: &str = "%F %T";
const DATE_FORMAT: &str = "%F";

pub fn format_time_millis(ts_millis: u64) -> String {
    OffsetDateTime::from_unix_timestamp_nanos((ts_millis as i128) * (*UNIX_TIME_UNIT_OFFSET))
        .format(time::Format::Custom(TIME_FORMAT.into()))
}

pub fn curr_time_millis() -> u64 {
    (OffsetDateTime::now_utc().unix_timestamp_nanos() / (*UNIX_TIME_UNIT_OFFSET)) as u64
}

pub fn curr_time_nanos() -> i128 {
    OffsetDateTime::now_utc().unix_timestamp_nanos()
}
