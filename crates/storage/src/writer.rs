use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Utc};
use clickhouse::Client;
use perp_radar_core::packet::StandardPacket;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;
use tokio::time::{interval, MissedTickBehavior};

use crate::batcher::BatchConfig;
use crate::rows::{truncate_to_minute, Feature1mRow, LatestPacketRow};
use crate::sink::PersistEvent;

#[derive(Debug, Default)]
pub struct FeatureMinuteDedupe {
    last_feature_minute_by_symbol: HashMap<String, DateTime<Utc>>,
}

impl FeatureMinuteDedupe {
    pub fn should_write(&mut self, packet: &StandardPacket) -> bool {
        let minute = truncate_to_minute(packet.ts);
        match self.last_feature_minute_by_symbol.get(&packet.symbol) {
            Some(previous) if *previous == minute => false,
            _ => {
                self.last_feature_minute_by_symbol
                    .insert(packet.symbol.clone(), minute);
                true
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct PendingRows {
    latest_packets: Vec<LatestPacketRow>,
    features_1m: Vec<Feature1mRow>,
    feature_dedupe: FeatureMinuteDedupe,
}

impl PendingRows {
    pub fn push_packet(&mut self, packet: &StandardPacket) -> Result<()> {
        self.latest_packets
            .push(LatestPacketRow::from_packet(packet)?);
        if self.feature_dedupe.should_write(packet) {
            self.features_1m.push(Feature1mRow::from_packet(packet)?);
        }
        Ok(())
    }

    pub fn latest_packets(&self) -> &[LatestPacketRow] {
        &self.latest_packets
    }

    pub fn features_1m(&self) -> &[Feature1mRow] {
        &self.features_1m
    }

    pub fn row_count(&self) -> usize {
        self.latest_packets.len() + self.features_1m.len()
    }

    pub fn is_empty(&self) -> bool {
        self.latest_packets.is_empty() && self.features_1m.is_empty()
    }

    pub fn drain_rows(&mut self) -> DrainedRows {
        DrainedRows {
            latest_packets: std::mem::take(&mut self.latest_packets),
            features_1m: std::mem::take(&mut self.features_1m),
        }
    }
}

#[derive(Debug, Default)]
pub struct DrainedRows {
    latest_packets: Vec<LatestPacketRow>,
    features_1m: Vec<Feature1mRow>,
}

impl DrainedRows {
    pub fn latest_packets(&self) -> &[LatestPacketRow] {
        &self.latest_packets
    }

    pub fn features_1m(&self) -> &[Feature1mRow] {
        &self.features_1m
    }
}

pub fn spawn_clickhouse_writer(
    client: Client,
    config: BatchConfig,
    mut receiver: Receiver<PersistEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut pending = PendingRows::default();
        let mut flush_interval =
            interval(std::time::Duration::from_millis(config.flush_interval_ms));
        flush_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                event = receiver.recv() => {
                    match event {
                        Some(PersistEvent::Packet(packet)) => {
                            if let Err(error) = pending.push_packet(&packet) {
                                tracing::warn!(%error, "failed to prepare persistence rows");
                                continue;
                            }
                            if config.should_flush(pending.row_count()) {
                                flush_pending(&client, &mut pending).await;
                            }
                        }
                        None => {
                            flush_pending(&client, &mut pending).await;
                            break;
                        }
                    }
                }
                _ = flush_interval.tick() => {
                    flush_pending(&client, &mut pending).await;
                }
            }
        }
    })
}

async fn flush_pending(client: &Client, pending: &mut PendingRows) {
    if pending.is_empty() {
        return;
    }

    let rows = pending.drain_rows();
    if let Err(error) = insert_latest_packets(client, rows.latest_packets()).await {
        tracing::warn!(%error, "failed to insert latest_packets rows");
    }
    if let Err(error) = insert_features_1m(client, rows.features_1m()).await {
        tracing::warn!(%error, "failed to insert features_1m rows");
    }
}

async fn insert_latest_packets(client: &Client, rows: &[LatestPacketRow]) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let mut insert = client.insert("latest_packets")?;
    for row in rows {
        insert.write(row).await?;
    }
    insert.end().await?;
    Ok(())
}

async fn insert_features_1m(client: &Client, rows: &[Feature1mRow]) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let mut insert = client.insert("features_1m")?;
    for row in rows {
        insert.write(row).await?;
    }
    insert.end().await?;
    Ok(())
}
