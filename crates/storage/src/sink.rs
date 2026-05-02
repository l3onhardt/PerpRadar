use perp_radar_core::packet::StandardPacket;
use tokio::sync::mpsc::{error::TrySendError, Sender};

#[derive(Debug, Clone)]
pub enum PersistEvent {
    Packet(StandardPacket),
}

#[derive(Clone, Debug, Default)]
pub struct StorageSink {
    sender: Option<Sender<PersistEvent>>,
}

impl StorageSink {
    pub fn disabled() -> Self {
        Self { sender: None }
    }

    pub fn channel(sender: Sender<PersistEvent>) -> Self {
        Self {
            sender: Some(sender),
        }
    }

    pub fn persist_packet(&self, packet: StandardPacket) {
        let Some(sender) = &self.sender else {
            return;
        };

        match sender.try_send(PersistEvent::Packet(packet)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                tracing::warn!("storage persistence queue full; dropping packet event");
            }
            Err(TrySendError::Closed(_)) => {
                tracing::warn!("storage persistence queue closed; dropping packet event");
            }
        }
    }
}
