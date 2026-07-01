use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use tokio::sync::{watch, Mutex};

use crate::{
    app::AppState,
    config::{PARALLEL_STREAMS, TEST_DURATION, TEST_URLS, WARMUP_DURATION},
};

pub async fn run_download_test(tx: watch::Sender<AppState>) -> Result<()> {
    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(PARALLEL_STREAMS)
        .tcp_nodelay(true)
        .user_agent("NetDash/0.1")
        .build()?;

    let total_bytes = Arc::new(AtomicU64::new(0));
    let measuring = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let last_error = Arc::new(Mutex::new(String::new()));

    let test_start = Instant::now();
    let measure_start = test_start + WARMUP_DURATION;
    let test_end = test_start + TEST_DURATION;

    for stream_index in 0..PARALLEL_STREAMS {
        let client = client.clone();
        let total_bytes = Arc::clone(&total_bytes);
        let measuring = Arc::clone(&measuring);
        let stop = Arc::clone(&stop);
        let last_error = Arc::clone(&last_error);

        tokio::spawn(async move {
            let mut url_index = stream_index % TEST_URLS.len();

            loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }

                let base_url = TEST_URLS[url_index];
                url_index = (url_index + 1) % TEST_URLS.len();

                let url = build_test_url(base_url);

                let response = match client.get(&url).send().await {
                    Ok(response) => response,
                    Err(err) => {
                        *last_error.lock().await =
                            format!("falha ao conectar em {base_url}: {err}");
                        continue;
                    }
                };

                let response = match response.error_for_status() {
                    Ok(response) => response,
                    Err(err) => {
                        *last_error.lock().await =
                            format!("resposta inválida de {base_url}: {err}");
                        continue;
                    }
                };

                let mut stream = response.bytes_stream();

                while let Some(chunk_result) = stream.next().await {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }

                    let chunk = match chunk_result {
                        Ok(chunk) => chunk,
                        Err(err) => {
                            *last_error.lock().await =
                                format!("erro lendo dados de {base_url}: {err}");
                            break;
                        }
                    };

                    if measuring.load(Ordering::Relaxed) {
                        total_bytes.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                    }
                }
            }
        });
    }

    let mut history: Vec<u64> = vec![0; 50];
    let mut last_bytes = 0u64;
    let mut last_tick = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_millis(200));

    loop {
        interval.tick().await;

        let now = Instant::now();

        if now >= measure_start {
            measuring.store(true, Ordering::Relaxed);
        }

        if now >= test_end {
            stop.store(true, Ordering::Relaxed);
            break;
        }

        let measured_elapsed = now
            .saturating_duration_since(measure_start)
            .as_secs_f64()
            .max(0.001);

        let current_bytes = total_bytes.load(Ordering::Relaxed);
        let bytes_delta = current_bytes.saturating_sub(last_bytes);
        let time_delta = now.duration_since(last_tick).as_secs_f64().max(0.001);

        let instant_mbps = (bytes_delta as f64 * 8.0) / time_delta / 1_000_000.0;
        let average_mbps = (current_bytes as f64 * 8.0) / measured_elapsed / 1_000_000.0;
        let total_mb = current_bytes as f64 / 1_000_000.0;

        last_bytes = current_bytes;
        last_tick = now;

        history.push(instant_mbps as u64);
        if history.len() > 50 {
            history.remove(0);
        }

        let _ = tx.send(AppState {
            download_mbps: instant_mbps,
            average_mbps,
            total_mb,
            elapsed_secs: now.duration_since(test_start).as_secs_f64(),
            running: true,
            done: false,
            history: history.clone(),
            error: None,
        });
    }

    tokio::time::sleep(Duration::from_millis(300)).await;

    let final_bytes = total_bytes.load(Ordering::Relaxed);
    let final_measured_secs = TEST_DURATION
        .checked_sub(WARMUP_DURATION)
        .unwrap_or(TEST_DURATION)
        .as_secs_f64();

    if final_bytes == 0 {
        let error = last_error.lock().await.clone();

        return Err(anyhow!(
            "nenhum dado foi baixado. Último erro: {}",
            if error.is_empty() {
                "nenhum erro específico capturado"
            } else {
                &error
            }
        ));
    }

    let final_mbps = (final_bytes as f64 * 8.0) / final_measured_secs / 1_000_000.0;
    let final_mb = final_bytes as f64 / 1_000_000.0;

    let _ = tx.send(AppState {
        download_mbps: final_mbps,
        average_mbps: final_mbps,
        total_mb: final_mb,
        elapsed_secs: TEST_DURATION.as_secs_f64(),
        running: false,
        done: true,
        history,
        error: None,
    });

    Ok(())
}

fn build_test_url(base_url: &str) -> String {
    if base_url.contains("speed.cloudflare.com") {
        base_url.to_string()
    } else {
        let separator = if base_url.contains('?') { "&" } else { "?" };

        format!(
            "{}{}cache_bust={}",
            base_url,
            separator,
            timestamp_nanos()
        )
    }
}

fn timestamp_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}