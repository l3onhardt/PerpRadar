#[derive(Debug, Clone, PartialEq)]
pub struct BookLevel {
    pub price: f64,
    pub qty: f64,
}

#[derive(Debug, Clone)]
pub struct PartialBook {
    pub symbol: String,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
}

impl PartialBook {
    pub fn new(symbol: impl Into<String>, bids: Vec<BookLevel>, asks: Vec<BookLevel>) -> Self {
        Self {
            symbol: symbol.into(),
            bids,
            asks,
        }
    }

    pub fn best_bid(&self) -> Option<&BookLevel> {
        self.bids.first()
    }

    pub fn best_ask(&self) -> Option<&BookLevel> {
        self.asks.first()
    }

    pub fn mid(&self) -> Option<f64> {
        Some((self.best_bid()?.price + self.best_ask()?.price) / 2.0)
    }

    pub fn spread_bp(&self) -> Option<f64> {
        let bid = self.best_bid()?;
        Some((self.best_ask()?.price - bid.price) / bid.price * 10_000.0)
    }

    pub fn imbalance_top_n(&self, n: usize) -> Option<f64> {
        let bid_qty: f64 = self.bids.iter().take(n).map(|level| level.qty).sum();
        let ask_qty: f64 = self.asks.iter().take(n).map(|level| level.qty).sum();
        let total = bid_qty + ask_qty;
        if total == 0.0 {
            return None;
        }
        Some((bid_qty - ask_qty) / total)
    }

    pub fn microprice_bp(&self) -> Option<f64> {
        let bid = self.best_bid()?;
        let ask = self.best_ask()?;
        let mid = self.mid()?;
        let microprice = (ask.price * bid.qty + bid.price * ask.qty) / (bid.qty + ask.qty);
        Some((microprice - mid) / mid * 10_000.0)
    }
}
