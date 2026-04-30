use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
pub struct TokenBucket {
    capacity: usize,
    tokens: AtomicUsize,
}

impl TokenBucket {
    pub fn new(tokens: usize) -> Self {
        Self {
            capacity: tokens,
            tokens: AtomicUsize::new(tokens),
        }
    }

    pub fn try_take(&self, tokens: usize) -> bool {
        self.tokens
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |available| {
                if available >= tokens {
                    Some(available - tokens)
                } else {
                    None
                }
            })
            .is_ok()
    }

    pub fn refill(&self, tokens: usize) {
        let _ = self
            .tokens
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |available| {
                Some(available.saturating_add(tokens).min(self.capacity))
            });
    }
}
