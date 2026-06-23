use chrono::{TimeZone, Utc};
use perp_radar_core::packet::{
    CarryBlock, ChartBlock, DerivativesBlock, EventsBlock, LegacyScoresBlock, LiquidityBlock,
    OrderflowBlock, PacketProfile, PriceBlock, ScoresBlock, StandardPacket, StructureBlock,
    UniverseBlock,
};
use perp_radar_core::quality::{QualityReason, QualityState};
use perp_radar_core::types::UniverseTier;
use perp_radar_storage::rows::{Feature1mRow, LatestPacketRow};
use perp_radar_storage::sink::{PersistEvent, StorageSink};
use perp_radar_storage::writer::{FeatureMinuteDedupe, PendingRows};

fn packet_at(minute: u32) -> StandardPacket {
    StandardPacket {
        packet_schema: "2.1".to_string(),
        ts: Utc.with_ymd_and_hms(2026, 5, 2, 7, minute, 42).unwrap(),
        symbol: "BTCUSDT".to_string(),
        rank: 1,
        profile: PacketProfile::Standard,
        universe: UniverseBlock {
            tier: UniverseTier::U2,
            active_n: 15,
            focus_n: 3,
        },
        price: PriceBlock {
            last: Some(64_000.0),
            mark: Some(64_001.0),
            index: Some(63_990.0),
            basis_bp: Some(1.7),
            ret_1m: Some(0.001),
            ret_5m: Some(0.005),
            ret_15m: None,
            ret_1h: Some(0.01),
        },
        chart: ChartBlock {
            rsi_14: Some(55.0),
            atr_pct: Some(0.02),
            macd_histogram: Some(1.2),
            adx_14: Some(21.0),
            bb_width: Some(0.04),
            ..ChartBlock::default()
        },
        liquidity: LiquidityBlock {
            book_mode: "full".to_string(),
            spread_bp: Some(0.5),
            i1: Some(0.1),
            i5: Some(0.2),
            ..LiquidityBlock::default()
        },
        carry: CarryBlock {
            funding_z_7d: Some(-0.8),
            ..CarryBlock::default()
        },
        events: EventsBlock::default(),
        structure: StructureBlock::default(),
        derivatives: DerivativesBlock::default(),
        orderflow: OrderflowBlock::default(),
        scores: ScoresBlock {
            tcs: Some(1.0),
            lri: None,
            dpi5: Some(0.2),
            dpi10: Some(0.3),
            csi: Some(-1.0),
            rpi: Some(0.4),
            vov: Some(0.5),
        },
        score_meta: Default::default(),
        legacy_scores: LegacyScoresBlock::default(),
        quality: QualityState {
            freshness_ms: 123,
            warm: true,
            kline_gap_1m: 0,
            book_mode: "full".to_string(),
            book_seq_ok: Some(true),
            book_depth_coverage_bp: Some(10.0),
            funding_history_points: 126,
            stale: false,
            reasons: vec![QualityReason::InsufficientFundingHistory],
        },
    }
}

#[test]
fn latest_packet_row_preserves_packet_json() {
    let packet = packet_at(51);

    let row = LatestPacketRow::from_packet(&packet).unwrap();

    assert_eq!(row.ts, packet.ts);
    assert_eq!(row.symbol, "BTCUSDT");
    assert_eq!(row.profile, "standard");
    assert_eq!(row.rank, 1);
    assert!(row.packet_json.contains("\"packet_schema\":\"2.1\""));
    assert!(row.packet_json.contains("\"symbol\":\"BTCUSDT\""));
}

#[test]
fn feature_row_maps_packet_fields_and_quality_json() {
    let packet = packet_at(51);

    let row = Feature1mRow::from_packet(&packet).unwrap();

    assert_eq!(row.ts, Utc.with_ymd_and_hms(2026, 5, 2, 7, 51, 0).unwrap());
    assert_eq!(row.symbol, "BTCUSDT");
    assert_eq!(row.price, Some(64_000.0));
    assert_eq!(row.ret_1m, Some(0.001));
    assert_eq!(row.ret_5m, Some(0.005));
    assert_eq!(row.ret_15m, None);
    assert_eq!(row.rsi14, Some(55.0));
    assert_eq!(row.lri, None);
    assert_eq!(row.dpi5, Some(0.2));
    assert_eq!(row.vov, Some(0.5));
    assert!(row.quality_json.contains("\"warm\":true"));
    assert!(row.quality_json.contains("insufficient_funding_history"));
}

#[tokio::test]
async fn storage_sink_enqueues_packet_events_without_clickhouse() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    let sink = StorageSink::channel(sender);
    let packet = packet_at(51);

    sink.persist_packet(packet.clone());

    let event = receiver.recv().await.expect("event is queued");
    match event {
        PersistEvent::Packet(received) => {
            assert_eq!(received.symbol, packet.symbol);
            assert_eq!(received.ts, packet.ts);
        }
    }
}

#[test]
fn feature_minute_dedupe_writes_once_per_symbol_minute() {
    let mut dedupe = FeatureMinuteDedupe::default();
    let packet = packet_at(51);
    let next_minute = packet_at(52);

    assert!(dedupe.should_write(&packet));
    assert!(!dedupe.should_write(&packet));
    assert!(dedupe.should_write(&next_minute));
}

#[test]
fn pending_rows_collect_latest_packets_and_deduped_features() {
    let mut pending = PendingRows::default();
    let first = packet_at(51);
    let same_minute = packet_at(51);
    let next_minute = packet_at(52);

    pending.push_packet(&first).unwrap();
    pending.push_packet(&same_minute).unwrap();
    pending.push_packet(&next_minute).unwrap();

    assert_eq!(pending.latest_packets().len(), 3);
    assert_eq!(pending.features_1m().len(), 2);
    assert_eq!(pending.row_count(), 5);
}

#[test]
fn pending_rows_keeps_feature_dedupe_after_take() {
    let mut pending = PendingRows::default();
    let first = packet_at(51);
    let same_minute = packet_at(51);

    pending.push_packet(&first).unwrap();
    let taken = pending.drain_rows();
    assert_eq!(taken.features_1m().len(), 1);

    pending.push_packet(&same_minute).unwrap();

    assert_eq!(pending.latest_packets().len(), 1);
    assert_eq!(pending.features_1m().len(), 0);
}

#[tokio::test]
async fn row_types_insert_into_clickhouse_datetime64_tables_when_available() {
    let client = clickhouse::Client::default()
        .with_url("http://127.0.0.1:8123")
        .with_user("perp_radar")
        .with_password("perp_radar");
    if client.query("SELECT 1").execute().await.is_err() {
        return;
    }

    client
        .query("DROP TABLE IF EXISTS perp_radar.persistence_contract_latest")
        .execute()
        .await
        .unwrap();
    client
        .query("DROP TABLE IF EXISTS perp_radar.persistence_contract_features")
        .execute()
        .await
        .unwrap();
    client
        .query(
            r#"
            CREATE TABLE perp_radar.persistence_contract_latest
            (
                ts DateTime64(3, 'UTC'),
                symbol String,
                profile String,
                rank UInt32,
                packet_json String
            )
            ENGINE = MergeTree
            ORDER BY (symbol, ts)
            "#,
        )
        .execute()
        .await
        .unwrap();
    client
        .query(
            r#"
            CREATE TABLE perp_radar.persistence_contract_features
            (
                ts DateTime64(3, 'UTC'),
                symbol String,
                price Nullable(Float64),
                ret_1m Nullable(Float64),
                ret_5m Nullable(Float64),
                ret_15m Nullable(Float64),
                atr_pct Nullable(Float64),
                rsi14 Nullable(Float64),
                macd_hist Nullable(Float64),
                adx14 Nullable(Float64),
                bb_width Nullable(Float64),
                funding_z_7d Nullable(Float64),
                basis_bp Nullable(Float64),
                spread_bp Nullable(Float64),
                i1 Nullable(Float64),
                i5 Nullable(Float64),
                tcs Nullable(Float64),
                lri Nullable(Float64),
                dpi5 Nullable(Float64),
                csi Nullable(Float64),
                rpi Nullable(Float64),
                vov Nullable(Float64),
                quality_json String
            )
            ENGINE = MergeTree
            ORDER BY (symbol, ts)
            "#,
        )
        .execute()
        .await
        .unwrap();

    let packet = packet_at(51);
    let latest = LatestPacketRow::from_packet(&packet).unwrap();
    let feature = Feature1mRow::from_packet(&packet).unwrap();

    let mut latest_insert = client
        .insert("perp_radar.persistence_contract_latest")
        .unwrap();
    latest_insert.write(&latest).await.unwrap();
    latest_insert.end().await.unwrap();

    let mut feature_insert = client
        .insert("perp_radar.persistence_contract_features")
        .unwrap();
    feature_insert.write(&feature).await.unwrap();
    feature_insert.end().await.unwrap();

    let latest_count = client
        .query("SELECT count() FROM perp_radar.persistence_contract_latest")
        .fetch_one::<u64>()
        .await
        .unwrap();
    let feature_count = client
        .query("SELECT count() FROM perp_radar.persistence_contract_features")
        .fetch_one::<u64>()
        .await
        .unwrap();

    assert_eq!(latest_count, 1);
    assert_eq!(feature_count, 1);

    client
        .query("DROP TABLE IF EXISTS perp_radar.persistence_contract_latest")
        .execute()
        .await
        .unwrap();
    client
        .query("DROP TABLE IF EXISTS perp_radar.persistence_contract_features")
        .execute()
        .await
        .unwrap();
}
