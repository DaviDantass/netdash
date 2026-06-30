use std::time::Duration;

pub const TEST_DURATION: Duration = Duration::from_secs(8);
pub const WARMUP_DURATION: Duration = Duration::from_millis(800);
pub const PARALLEL_STREAMS: usize = 8;

pub const TEST_URLS: [&str; 4] = [
    "https://speed.cloudflare.com/__down?bytes=1000000000",
    "https://cachefly.cachefly.net/100mb.test",
    "https://proof.ovh.net/files/1Gb.dat",
    "https://ipv4.download.thinkbroadband.com/1GB.zip",
];