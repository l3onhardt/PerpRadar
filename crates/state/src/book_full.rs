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
    bootstrapped: bool,
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
            bootstrapped: false,
            bids: levels_to_map(bids),
            asks: levels_to_map(asks),
        }
    }

    pub fn apply_delta(&mut self, delta: BookDelta) -> Result<(), FullBookError> {
        let sequence_ok = if self.bootstrapped {
            delta.previous_final_update_id == self.last_update_id
        } else {
            delta.first_update_id <= self.last_update_id
                && delta.final_update_id >= self.last_update_id
        };

        if !sequence_ok {
            self.seq_ok = false;
            return Err(FullBookError::SequenceGap);
        }

        apply_levels(&mut self.bids, delta.bids);
        apply_levels(&mut self.asks, delta.asks);
        self.last_update_id = delta.final_update_id;
        self.seq_ok = true;
        self.bootstrapped = true;
        Ok(())
    }

    pub fn seq_ok(&self) -> bool {
        self.seq_ok
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub fn best_bid(&self) -> Option<f64> {
        self.bids.keys().next_back().map(|key| key_to_price(*key))
    }

    pub fn best_ask(&self) -> Option<f64> {
        self.asks.keys().next().map(|key| key_to_price(*key))
    }

    pub fn mid(&self) -> Option<f64> {
        Some((self.best_bid()? + self.best_ask()?) / 2.0)
    }

    pub fn visible_liquidity_usd(&self, max_distance_bp: f64) -> Option<f64> {
        if !max_distance_bp.is_finite() || max_distance_bp < 0.0 {
            return None;
        }
        let mid = self.mid()?;
        let bid_floor = mid * (1.0 - max_distance_bp / 10_000.0);
        let ask_ceiling = mid * (1.0 + max_distance_bp / 10_000.0);
        let bid_notional = self
            .bids
            .iter()
            .rev()
            .map(|(price, qty)| (key_to_price(*price), *qty))
            .take_while(|(price, _)| *price >= bid_floor)
            .map(|(price, qty)| price * qty)
            .sum::<f64>();
        let ask_notional = self
            .asks
            .iter()
            .map(|(price, qty)| (key_to_price(*price), *qty))
            .take_while(|(price, _)| *price <= ask_ceiling)
            .map(|(price, qty)| price * qty)
            .sum::<f64>();
        Some(bid_notional + ask_notional)
    }

    pub fn slippage_bp_for_notional(&self, notional_usd: f64, buy: bool) -> Option<f64> {
        if !notional_usd.is_finite() || notional_usd <= 0.0 {
            return None;
        }
        let mid = self.mid()?;
        let mut remaining = notional_usd;
        let mut acquired_qty = 0.0;
        let mut spent_notional = 0.0;

        if buy {
            for (price_key, qty) in &self.asks {
                fill_level(
                    key_to_price(*price_key),
                    *qty,
                    &mut remaining,
                    &mut acquired_qty,
                    &mut spent_notional,
                );
                if remaining <= 0.0 {
                    break;
                }
            }
        } else {
            for (price_key, qty) in self.bids.iter().rev() {
                fill_level(
                    key_to_price(*price_key),
                    *qty,
                    &mut remaining,
                    &mut acquired_qty,
                    &mut spent_notional,
                );
                if remaining <= 0.0 {
                    break;
                }
            }
        }

        if remaining > 0.0 || acquired_qty == 0.0 {
            return None;
        }

        let average_price = spent_notional / acquired_qty;
        Some((average_price - mid).abs() / mid * 10_000.0)
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

fn key_to_price(key: i64) -> f64 {
    key as f64 / 100_000_000.0
}

fn fill_level(
    price: f64,
    qty: f64,
    remaining: &mut f64,
    acquired_qty: &mut f64,
    spent_notional: &mut f64,
) {
    if qty <= 0.0 || *remaining <= 0.0 {
        return;
    }

    let level_notional = price * qty;
    let used_notional = level_notional.min(*remaining);
    let used_qty = used_notional / price;
    *remaining -= used_notional;
    *acquired_qty += used_qty;
    *spent_notional += used_notional;
}
