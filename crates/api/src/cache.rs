use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use perp_radar_core::packet::StandardPacket;

#[derive(Debug, Clone, Default)]
pub struct PacketCache {
    packets: Arc<RwLock<HashMap<String, StandardPacket>>>,
}

impl PacketCache {
    pub fn upsert(&self, mut packet: StandardPacket) {
        packet.symbol = canonical_symbol(&packet.symbol);
        self.write_packets().insert(packet.symbol.clone(), packet);
    }

    pub fn retain_symbols<'a>(&self, symbols: impl IntoIterator<Item = &'a String>) {
        let allowed = symbols
            .into_iter()
            .map(|symbol| canonical_symbol(symbol))
            .collect::<std::collections::HashSet<_>>();
        self.write_packets()
            .retain(|symbol, _| allowed.contains(symbol));
    }

    pub fn get(&self, symbol: &str) -> Option<StandardPacket> {
        self.read_packets().get(&canonical_symbol(symbol)).cloned()
    }

    pub fn top(&self, limit: usize) -> Vec<StandardPacket> {
        let mut packets: Vec<_> = self.read_packets().values().cloned().collect();
        packets.sort_by(|left, right| {
            left.rank
                .cmp(&right.rank)
                .then_with(|| left.symbol.cmp(&right.symbol))
        });
        packets.truncate(limit);
        packets
    }

    pub fn len(&self) -> usize {
        self.read_packets().len()
    }

    fn read_packets(&self) -> RwLockReadGuard<'_, HashMap<String, StandardPacket>> {
        self.packets
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write_packets(&self) -> RwLockWriteGuard<'_, HashMap<String, StandardPacket>> {
        self.packets
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn canonical_symbol(symbol: &str) -> String {
    symbol.to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use chrono::{TimeZone, Utc};
    use perp_radar_core::packet::{
        CarryBlock, ChartBlock, EventsBlock, LegacyScoresBlock, LiquidityBlock, PacketProfile,
        PriceBlock, ScoresBlock, UniverseBlock,
    };
    use perp_radar_core::quality::QualityState;
    use perp_radar_core::types::UniverseTier;

    use super::*;

    fn packet(symbol: &str, rank: usize) -> StandardPacket {
        StandardPacket {
            packet_schema: "2.1".to_string(),
            ts: Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap(),
            symbol: symbol.to_string(),
            rank,
            profile: PacketProfile::Standard,
            universe: UniverseBlock {
                tier: UniverseTier::U2,
                active_n: 1,
                focus_n: 1,
            },
            price: PriceBlock::default(),
            chart: ChartBlock::default(),
            liquidity: LiquidityBlock::default(),
            carry: CarryBlock::default(),
            events: EventsBlock::default(),
            scores: ScoresBlock::default(),
            score_meta: std::collections::BTreeMap::new(),
            legacy_scores: LegacyScoresBlock::default(),
            quality: QualityState::cold("partial20"),
        }
    }

    #[test]
    fn cache_keys_are_ascii_uppercase() {
        let cache = PacketCache::default();
        cache.upsert(packet("btcusdt", 1));

        assert_eq!(cache.get("BTCUSDT").unwrap().symbol, "BTCUSDT");
        assert_eq!(cache.get("btcusdt").unwrap().symbol, "BTCUSDT");
    }

    #[test]
    fn poisoned_lock_does_not_panic_for_cache_operations() {
        let cache = PacketCache::default();
        let poisoned = cache.clone();
        let _ = catch_unwind(AssertUnwindSafe(move || {
            let _guard = poisoned.packets.write().unwrap();
            panic!("poison packet cache lock");
        }));

        assert!(catch_unwind(AssertUnwindSafe(|| cache.upsert(packet("BTCUSDT", 1)))).is_ok());
        assert!(catch_unwind(AssertUnwindSafe(|| cache.get("BTCUSDT"))).is_ok());
        assert!(catch_unwind(AssertUnwindSafe(|| cache.top(1))).is_ok());
        assert_eq!(cache.get("BTCUSDT").unwrap().symbol, "BTCUSDT");
    }
}
