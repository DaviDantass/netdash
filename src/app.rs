#[derive(Clone, Debug)]
pub struct AppState {
    pub download_mbps: f64,
    pub average_mbps: f64,
    pub total_mb: f64,
    pub elapsed_secs: f64,
    pub running: bool,
    pub done: bool,
    pub history: Vec<u64>,
    pub error: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            download_mbps: 0.0,
            average_mbps: 0.0,
            total_mb: 0.0,
            elapsed_secs: 0.0,
            running: true,
            done: false,
            history: vec![0; 50],
            error: None,
        }
    }
}