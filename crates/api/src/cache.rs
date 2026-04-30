use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use perp_radar_core::packet::StandardPacket;

#[derive(Debug, Clone, Default)]
pub struct PacketCache {
    packets: Arc<RwLock<HashMap<String, StandardPacket>>>,
}

impl PacketCache {
    pub fn upsert(&self, packet: StandardPacket) {
        self.packets
            .write()
            .expect("packet cache lock poisoned")
            .insert(packet.symbol.clone(), packet);
    }

    pub fn get(&self, symbol: &str) -> Option<StandardPacket> {
        self.packets
            .read()
            .expect("packet cache lock poisoned")
            .get(symbol)
            .cloned()
    }

    pub fn top(&self, limit: usize) -> Vec<StandardPacket> {
        let mut packets: Vec<_> = self
            .packets
            .read()
            .expect("packet cache lock poisoned")
            .values()
            .cloned()
            .collect();
        packets.sort_by_key(|packet| packet.rank);
        packets.truncate(limit);
        packets
    }
}
