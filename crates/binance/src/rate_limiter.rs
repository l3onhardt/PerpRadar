use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
pub struct TokenBucket {
    tokens: AtomicUsize,
}

impl TokenBucket {
    pub fn new(tokens: usize) -> Self {
        Self {
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
}
