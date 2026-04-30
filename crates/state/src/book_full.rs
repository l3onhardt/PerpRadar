use std::collections::BTreeMap;

use thiserror::Error;

use crate::book_partial::BookLevel;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FullBookError {
    #[error("full book sequence gap")]
    SequenceGap,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LevelDelta {
    pub price: f64,
    pub qty: f64,
}

#[derive(Debug, Clone)]
pub struct BookDelta {
    pub first_update_id: u64,
    pub final_update_id: u64,
    pub previous_final_update_id: u64,
    pub bids: Vec<LevelDelta>,
    pub asks: Vec<LevelDelta>,
}

#[derive(Debug, Clone)]
pub struct FullBook {
    symbol: String,
    last_update_id: u64,
    seq_ok: bool,
    bids: BTreeMap<i64, f64>,
    asks: BTreeMap<i64, f64>,
}

impl FullBook {
    pub fn from_snapshot(
        symbol: impl Into<String>,
        last_update_id: u64,
        bids: Vec<BookLevel>,
        asks: Vec<BookLevel>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            last_update_id,
            seq_ok: true,
            bids: levels_to_map(bids),
            asks: levels_to_map(asks),
        }
    }

    pub fn apply_delta(&mut self, delta: BookDelta) -> Result<(), FullBookError> {
        if delta.previous_final_update_id != self.last_update_id {
            self.seq_ok = false;
            return Err(FullBookError::SequenceGap);
        }

        apply_levels(&mut self.bids, delta.bids);
        apply_levels(&mut self.asks, delta.asks);
        self.last_update_id = delta.final_update_id;
        self.seq_ok = true;
        Ok(())
    }

    pub fn seq_ok(&self) -> bool {
        self.seq_ok
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }
}

fn levels_to_map(levels: Vec<BookLevel>) -> BTreeMap<i64, f64> {
    let mut map = BTreeMap::new();
    for level in levels {
        map.insert(price_key(level.price), level.qty);
    }
    map
}

fn apply_levels(map: &mut BTreeMap<i64, f64>, levels: Vec<LevelDelta>) {
    for level in levels {
        let key = price_key(level.price);
        if level.qty == 0.0 {
            map.remove(&key);
        } else {
            map.insert(key, level.qty);
        }
    }
}

fn price_key(price: f64) -> i64 {
    (price * 100_000_000.0).round() as i64
}
