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

    pub fn should_flush(&self, pending_rows: usize) -> bool {
        pending_rows > 0 && self.max_rows > 0 && pending_rows >= self.max_rows
    }
}
