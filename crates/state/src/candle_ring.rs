use std::collections::VecDeque;

use perp_radar_core::types::Candle;

#[derive(Debug, Clone)]
pub struct CandleRing {
    capacity: usize,
    items: VecDeque<Candle>,
}

impl CandleRing {
    pub fn new(capacity: usize) -> Self {
        assert!(
            capacity > 0,
            "candle ring capacity must be greater than zero"
        );
        Self {
            capacity,
            items: VecDeque::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, candle: Candle) {
        if self.items.len() == self.capacity {
            self.items.pop_front();
        }
        self.items.push_back(candle);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn last(&self) -> Option<&Candle> {
        self.items.back()
    }

    pub fn items(&self) -> Vec<Candle> {
        self.items.iter().cloned().collect()
    }
}
