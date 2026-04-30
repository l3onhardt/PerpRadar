#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchConfig {
    pub max_rows: usize,
    pub flush_interval_ms: u64,
}

impl BatchConfig {
    pub fn new(max_rows: usize, flush_interval_ms: u64) -> Self {
        Self {
            max_rows,
            flush_interval_ms,
        }
    }

    pub fn should_flush(&self, rows: usize, elapsed_ms: u64) -> bool {
        rows >= self.max_rows || (rows > 0 && elapsed_ms >= self.flush_interval_ms)
    }
}
