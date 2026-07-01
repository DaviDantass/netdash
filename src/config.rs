use std::time::Duration;

pub const TEST_DURATION: Duration = Duration::from_secs(10);
pub const WARMUP_DURATION: Duration = Duration::from_millis(1000);
pub const PARALLEL_STREAMS: usize = 4;

pub const TEST_URLS: [&str; 1] = [
    "https://cachefly.cachefly.net/100mb.test",
];