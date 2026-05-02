use std::collections::BTreeMap;

use chrono::Utc;
use perp_radar_core::packet::{
    CarryBlock, ChartBlock, EventsBlock, LegacyScoresBlock, LiquidityBlock, PacketProfile,
    PriceBlock, ScoreMeta, ScoresBlock, StandardPacket, UniverseBlock,
};
use perp_radar_core::quality::QualityReason;
use perp_radar_core::types::{Candle, UniverseTier};
use perp_radar_state::score_history::{RingWindow, ScoreHistoryState};
use perp_radar_state::symbol_state::SymbolState;

use crate::ta::{return_pct, technical_snapshot};

pub fn build_standard_packet(
    state: &SymbolState,
    rank: usize,
    active_n: usize,
    focus_n: usize,
) -> StandardPacket {
    build_standard_packet_with_funding_interval(state, rank, active_n, focus_n, 8)
}

pub fn build_standard_packet_with_funding_interval(
    state: &SymbolState,
    rank: usize,
    active_n: usize,
    focus_n: usize,
    funding_interval_hours: u32,
) -> StandardPacket {
    let candles = state.candles_1m.items();
    let last = candles.last();
    let mut quality = state.quality.clone();
    let technicals = technical_snapshot(&candles);
    let last_price = state.last_price.or_else(|| last.map(|candle| candle.close));
    let price = price_block(state, &candles, last_price);
    let chart = chart_block(&candles, technicals.as_ref(), &state.score_history);
    let liquidity = liquidity_block(state);
    let carry = carry_block(state, funding_interval_hours);
    let events = events_block(state, &candles);
    let score_eval = evaluate_formal_scores(state, &price, &chart, &liquidity, &carry);

    if candles.len() <= 5 {
        quality.add_reason(QualityReason::InsufficientKlineHistory);
    }
    if state.mark_price.is_none() {
        quality.add_reason(QualityReason::MissingMarkPrice);
    }
    if state.index_price.is_none() {
        quality.add_reason(QualityReason::MissingIndexPrice);
    }
    if state.funding_rate.is_none() {
        quality.add_reason(QualityReason::InsufficientFundingHistory);
    }

    StandardPacket {
        packet_schema: "2.1".to_string(),
        ts: Utc::now(),
        symbol: state.symbol.clone(),
        rank,
        profile: PacketProfile::Standard,
        universe: UniverseBlock {
            tier: UniverseTier::U2,
            active_n,
            focus_n,
        },
        price,
        chart,
        liquidity,
        carry,
        events,
        scores: score_eval.scores,
        score_meta: score_eval.meta,
        legacy_scores: legacy_scores_block(state, &candles),
        quality,
    }
}

fn price_block(state: &SymbolState, candles: &[Candle], last_price: Option<f64>) -> PriceBlock {
    PriceBlock {
        last: last_price,
        mark: state.mark_price,
        index: state.index_price,
        basis_bp: basis_bp(state.mark_price, state.index_price),
        ret_1m: tail_return(candles, 1),
        ret_5m: tail_return(candles, 5),
        ret_15m: tail_return(candles, 15),
        ret_1h: tail_return(candles, 60),
    }
}

fn chart_block(
    candles: &[Candle],
    technicals: Option<&crate::ta::TechnicalSnapshot>,
    history: &ScoreHistoryState,
) -> ChartBlock {
    ChartBlock {
        signature: chart_signature(candles),
        regime: technicals.and_then(|snapshot| snapshot.regime.clone()),
        ema_20: technicals.and_then(|snapshot| snapshot.ema_20),
        ema_50: technicals.and_then(|snapshot| snapshot.ema_50),
        ema_200: ema_last_from_candles(candles, 200),
        ema50_slope: ema_slope_from_candles(candles, 50, 10),
        rsi_14: technicals.and_then(|snapshot| snapshot.rsi_14),
        macd_histogram: technicals.and_then(|snapshot| snapshot.macd_histogram),
        atr_pct: technicals.and_then(|snapshot| snapshot.atr_pct),
        atr_1h_pct: history.latest_atr_1h_pct,
        atr_1h_pct_prev: history.latest_atr_1h_pct_prev,
        atr_1h_pct_delta_ratio: history.latest_atr_delta_ratio,
        bb_width: technicals.and_then(|snapshot| snapshot.bb_width),
        bb_width_pctile: history.latest_bb_width_pctile,
        adx_14: technicals.and_then(|snapshot| snapshot.adx_14),
        vwap_20: technicals.and_then(|snapshot| snapshot.vwap_20),
        cmf_20: technicals.and_then(|snapshot| snapshot.cmf_20),
    }
}

fn liquidity_block(state: &SymbolState) -> LiquidityBlock {
    let trusted_full_book = trusted_full_book(state);

    LiquidityBlock {
        book_mode: state.quality.book_mode.clone(),
        spread_bp: trusted_full_book
            .and_then(|book| book.spread_bp())
            .or_else(|| state.partial_book.as_ref().and_then(|book| book.spread_bp())),
        i1: trusted_full_book
            .and_then(|book| book.notional_imbalance_top_n(1))
            .or_else(|| {
                state
                    .partial_book
                    .as_ref()
                    .and_then(|book| book.imbalance_top_n(1))
            }),
        i5: trusted_full_book
            .and_then(|book| book.notional_imbalance_top_n(5))
            .or_else(|| {
                state
                    .partial_book
                    .as_ref()
                    .and_then(|book| book.imbalance_top_n(5))
            }),
        microprice_bp: trusted_full_book
            .and_then(|book| book.microprice_bp())
            .or_else(|| {
                state
                    .partial_book
                    .as_ref()
                    .and_then(|book| book.microprice_bp())
            }),
        liq_5bp_usd: state
            .full_book
            .as_ref()
            .and_then(|book| book.visible_liquidity_usd(5.0)),
        liq_10bp_usd: state
            .full_book
            .as_ref()
            .and_then(|book| book.visible_liquidity_usd(10.0)),
        slip_10000_buy_bp: state
            .full_book
            .as_ref()
            .and_then(|book| book.slippage_bp_for_notional(10_000.0, true)),
        slip_10000_sell_bp: state
            .full_book
            .as_ref()
            .and_then(|book| book.slippage_bp_for_notional(10_000.0, false)),
    }
}

fn legacy_top5_imbalance(state: &SymbolState) -> Option<f64> {
    trusted_full_book(state)
        .and_then(|book| book.notional_imbalance_top_n(5))
        .or_else(|| {
            state
                .partial_book
                .as_ref()
                .and_then(|book| book.imbalance_top_n(5))
        })
}

fn carry_block(state: &SymbolState, funding_interval_hours: u32) -> CarryBlock {
    CarryBlock {
        funding_now: state.funding_rate,
        funding_unit: state
            .funding_rate
            .map(|_| format!("{funding_interval_hours}h_estimate")),
        funding_interval_hours: state.funding_rate.map(|_| funding_interval_hours),
        funding_z_7d: state
            .funding_rate
            .and_then(|rate| crate::funding::z_score(&state.funding_history, rate)),
        next_funding_time: state.next_funding_time,
    }
}

fn events_block(state: &SymbolState, candles: &[Candle]) -> EventsBlock {
    EventsBlock {
        liq_1m_usd: liquidation_total(state, 60_000),
        liq_5m_usd: liquidation_total(state, 300_000),
        liq_15m_usd: liquidation_total(state, 900_000),
        liq_side: dominant_liquidation_side(state),
        volume_spike_z: volume_spike_z(candles),
    }
}

fn basis_bp(mark: Option<f64>, index: Option<f64>) -> Option<f64> {
    let mark = mark?;
    let index = index?;
    if !mark.is_finite() || !index.is_finite() || index == 0.0 {
        return None;
    }
    Some((mark - index) / index * 10_000.0)
}

fn tail_return(candles: &[Candle], minutes: usize) -> Option<f64> {
    let end = candles.last()?.close;
    let start = candles
        .get(candles.len().checked_sub(minutes + 1)?)
        .map(|candle| candle.close)?;
    return_pct(start, end)
}

fn chart_signature(candles: &[Candle]) -> Option<String> {
    if candles.is_empty() {
        return None;
    }

    let signature_candles = &candles[candles.len().saturating_sub(12)..];
    let colors = signature_candles
        .iter()
        .map(candle_color)
        .collect::<Vec<_>>()
        .join(",");
    Some(format!("1m:{colors}"))
}

fn candle_color(candle: &Candle) -> &'static str {
    if candle.close > candle.open {
        "G"
    } else if candle.close < candle.open {
        "R"
    } else {
        "DOJI"
    }
}

fn liquidation_total(state: &SymbolState, window_ms: i64) -> Option<f64> {
    let latest = state
        .liquidations
        .iter()
        .map(|event| event.event_time_ms)
        .max()?;
    Some(
        state
            .liquidations
            .iter()
            .filter(|event| latest - event.event_time_ms <= window_ms)
            .map(|event| event.notional_usd)
            .sum(),
    )
}

fn dominant_liquidation_side(state: &SymbolState) -> Option<String> {
    let latest = state
        .liquidations
        .iter()
        .map(|event| event.event_time_ms)
        .max()?;
    let mut long_notional = 0.0;
    let mut short_notional = 0.0;
    for event in state
        .liquidations
        .iter()
        .filter(|event| latest - event.event_time_ms <= 300_000)
    {
        match event.side.as_str() {
            "SELL" => long_notional += event.notional_usd,
            "BUY" => short_notional += event.notional_usd,
            _ => {}
        }
    }
    if long_notional > short_notional {
        Some("long".to_string())
    } else if short_notional > long_notional {
        Some("short".to_string())
    } else {
        None
    }
}

fn volume_spike_z(candles: &[Candle]) -> Option<f64> {
    let current = candles.last()?.volume_quote;
    if candles.len() < 3 {
        return None;
    }
    let history = candles[..candles.len() - 1]
        .iter()
        .rev()
        .take(20)
        .map(|candle| candle.volume_quote)
        .collect::<Vec<_>>();
    crate::ta::volume_z_score(&history, current)
}

#[derive(Debug, Clone, Copy, Default)]
struct LegacyCandidateComponents {
    candidate_score: Option<f64>,
    liquidation_event_score: Option<f64>,
    compression_score: Option<f64>,
    momentum_abs_score: Option<f64>,
    volume_spike_z: Option<f64>,
}

#[derive(Debug, Clone)]
struct ScoreEvaluation {
    scores: ScoresBlock,
    meta: BTreeMap<String, ScoreMeta>,
}

#[derive(Debug, Clone, Copy)]
struct ScoreConfig {
    min_samples: usize,
    z_clip: f64,
    slip_notional_usd: f64,
}

impl Default for ScoreConfig {
    fn default() -> Self {
        Self {
            min_samples: 30,
            z_clip: 5.0,
            slip_notional_usd: 10_000.0,
        }
    }
}

fn evaluate_formal_scores(
    state: &SymbolState,
    price: &PriceBlock,
    chart: &ChartBlock,
    liquidity: &LiquidityBlock,
    carry: &CarryBlock,
) -> ScoreEvaluation {
    let config = ScoreConfig::default();
    let lri = evaluate_lri(state, &config);
    let tcs = evaluate_tcs(state, price, chart, &config);
    let dpi5 = evaluate_dpi(state, 5);
    let dpi10 = evaluate_dpi(state, 10);
    let csi = evaluate_csi(state, carry, price, &config);
    let rpi = evaluate_rpi(state, chart, carry, price, liquidity, &config);
    let vov = evaluate_vov(state, chart, &config);

    let mut meta = BTreeMap::new();
    meta.insert("CSI".to_string(), csi.meta);
    meta.insert("DPI10".to_string(), dpi10.meta);
    meta.insert("DPI5".to_string(), dpi5.meta);
    meta.insert("LRI".to_string(), lri.meta);
    meta.insert("RPI".to_string(), rpi.meta);
    meta.insert("TCS".to_string(), tcs.meta);
    meta.insert("VoV".to_string(), vov.meta);

    ScoreEvaluation {
        scores: ScoresBlock {
            tcs: tcs.value,
            lri: lri.value,
            dpi5: dpi5.value,
            dpi10: dpi10.value,
            csi: csi.value,
            rpi: rpi.value,
            vov: vov.value,
        },
        meta,
    }
}

fn trusted_full_book(state: &SymbolState) -> Option<&perp_radar_state::book_full::FullBook> {
    state
        .full_book
        .as_ref()
        .filter(|_| state.quality.book_mode == "full" && state.quality.book_seq_ok == Some(true))
}

fn legacy_scores_block(state: &SymbolState, candles: &[Candle]) -> LegacyScoresBlock {
    let components = legacy_candidate_components(state, candles);
    LegacyScoresBlock {
        candidate_score: components.candidate_score,
        liquidation_event_score: components.liquidation_event_score,
        compression_score: components.compression_score,
        momentum_abs_score: components.momentum_abs_score,
        volume_spike_z: components.volume_spike_z,
        notional_imbalance_i5: legacy_top5_imbalance(state),
    }
}

fn legacy_candidate_components(
    state: &SymbolState,
    candles: &[Candle],
) -> LegacyCandidateComponents {
    let ret_15m_abs = tail_return(candles, 15)
        .or_else(|| tail_return(candles, 5))
        .map(f64::abs);
    let liquidity_quality = crate::liquidity::liquidity_quality(
        state
            .partial_book
            .as_ref()
            .and_then(|book| book.spread_bp()),
        state.quality.book_depth_coverage_bp,
    );
    let technicals = technical_snapshot(candles);
    let volume_z = volume_spike_z(candles);
    let lri = liquidation_total(state, 300_000).map(|value| (value / 1_000_000.0).min(3.0));
    let funding_abs = state
        .funding_rate
        .map(|rate| (rate.abs() / 0.0001).min(5.0));
    let squeeze_or_breakout = technicals
        .as_ref()
        .and_then(|snapshot| snapshot.bb_width)
        .map(|width| (0.1 - width).max(0.0) * 10.0);
    let tcs = crate::scores::composite_candidate_score(&crate::scores::ScoreInputs {
        volume_accel_z: volume_z.or(Some(0.0)),
        ret_15m_z_abs: ret_15m_abs,
        atr_pctile: technicals
            .as_ref()
            .and_then(|snapshot| snapshot.atr_pct)
            .or_else(|| tail_return(candles, 5).map(f64::abs)),
        funding_z_abs: funding_abs,
        liquidation_event_score: Some(lri.unwrap_or(0.0)),
        squeeze_or_breakout_score: squeeze_or_breakout.or(Some(0.0)),
        liquidity_quality,
    });

    LegacyCandidateComponents {
        candidate_score: tcs,
        liquidation_event_score: lri,
        compression_score: squeeze_or_breakout,
        momentum_abs_score: ret_15m_abs,
        volume_spike_z: volume_z,
    }
}

#[derive(Debug, Clone)]
struct ScoreResult {
    value: Option<f64>,
    meta: ScoreMeta,
}

fn evaluate_lri(state: &SymbolState, config: &ScoreConfig) -> ScoreResult {
    let mut missing = lri_source_missing_reasons(state, config.slip_notional_usd);
    let history = &state.score_history;
    let neg_spread = history.latest_neg_spread_bp;
    let liq_5bp = history.latest_liq_5bp_usd;
    let neg_slip = history.latest_neg_slip_bp;
    let spread_stats = neg_spread.and_then(|current| {
        robust_component(&history.neg_spread_bp, current, config.min_samples, config.z_clip)
    });
    let liq_stats = liq_5bp.and_then(|current| {
        robust_component(&history.liq_5bp_usd, current, config.min_samples, config.z_clip)
    });
    let slip_stats = neg_slip.and_then(|current| {
        robust_component(&history.neg_slip_bp, current, config.min_samples, config.z_clip)
    });
    if missing.is_empty() && (spread_stats.is_none() || liq_stats.is_none() || slip_stats.is_none())
    {
        missing.push("component_window_insufficient".to_string());
    }

    let raw = match (spread_stats, liq_stats, slip_stats) {
        (Some(spread), Some(liq), Some(slip)) if missing.is_empty() => {
            Some(0.4 * spread.z + 0.3 * liq.z + 0.3 * slip.z)
        }
        _ => None,
    };
    let final_stats = raw.and_then(|value| {
        robust_component(&history.lri_raw, value, config.min_samples, config.z_clip)
    });
    if raw.is_some() && final_stats.is_none() {
        missing.push("lri_raw_window_insufficient".to_string());
    }
    let value = final_stats.map(|stats| stats.z);
    ScoreResult {
        value,
        meta: ScoreMeta {
            available: value.is_some(),
            formula: Some(
                "robust_z(0.4*z(-spread_bp)+0.3*z(liq_5bp_usd)+0.3*z(-slip_bp))"
                    .to_string(),
            ),
            direction: Some(
                "higher means stronger observed liquidity / lower immediate execution friction under the defined formula"
                    .to_string(),
            ),
            book_source: Some("full".to_string()),
            slip_notional_usd: Some(config.slip_notional_usd),
            raw,
            z: value,
            components: serde_json::json!({
                "neg_spread_bp": component_json(neg_spread, spread_stats),
                "liq_5bp_usd": component_json(liq_5bp, liq_stats),
                "neg_slip_bp": component_json(neg_slip, slip_stats),
            }),
            missing,
        },
    }
}

fn lri_source_missing_reasons(state: &SymbolState, slip_notional_usd: f64) -> Vec<String> {
    let mut missing = Vec::new();
    if state.quality.book_mode != "full" {
        missing.push("book_not_full".to_string());
    }
    if state.quality.book_seq_ok != Some(true) {
        missing.push("book_seq_not_ok".to_string());
    }
    if trusted_full_book(state).and_then(|book| book.spread_bp()).is_none() {
        missing.push("spread_bp_missing".to_string());
    }
    if trusted_full_book(state)
        .and_then(|book| book.visible_liquidity_usd(5.0))
        .is_none()
    {
        missing.push("liq_5bp_usd_missing".to_string());
    }
    if trusted_full_book(state)
        .and_then(|book| book.slippage_bp_for_notional(slip_notional_usd, true))
        .is_none()
    {
        missing.push("slip_buy_missing".to_string());
    }
    if trusted_full_book(state)
        .and_then(|book| book.slippage_bp_for_notional(slip_notional_usd, false))
        .is_none()
    {
        missing.push("slip_sell_missing".to_string());
    }
    missing
}

fn evaluate_tcs(
    state: &SymbolState,
    price: &PriceBlock,
    chart: &ChartBlock,
    config: &ScoreConfig,
) -> ScoreResult {
    let history = &state.score_history;
    let mut missing = Vec::new();
    let close = price.last;
    let ema200 = chart.ema_200;
    let ema50_slope = chart.ema50_slope;
    let bb_width_pctile = chart.bb_width_pctile;
    let adx14 = chart.adx_14;
    push_missing(&mut missing, "close_missing", close);
    push_missing(&mut missing, "ema200_missing", ema200);
    push_missing(&mut missing, "ema50_slope_missing", ema50_slope);
    push_missing(&mut missing, "bb_width_pctile_missing", bb_width_pctile);
    push_missing(&mut missing, "adx14_missing", adx14);

    let adx_stats = adx14.and_then(|current| {
        robust_component(&history.adx14, current, config.min_samples, config.z_clip)
    });
    let slope_stats = ema50_slope.and_then(|current| {
        robust_component(&history.ema50_slope, current, config.min_samples, config.z_clip)
    });
    let bb_stats = bb_width_pctile.and_then(|current| {
        robust_component(
            &history.bb_width_pctile,
            current,
            config.min_samples,
            config.z_clip,
        )
    });
    if missing.is_empty() && (adx_stats.is_none() || slope_stats.is_none() || bb_stats.is_none()) {
        missing.push("component_window_insufficient".to_string());
    }
    let value = match (close, ema200, adx_stats, slope_stats, bb_stats) {
        (Some(close), Some(ema200), Some(adx), Some(slope), Some(bb)) if missing.is_empty() => {
            Some(adx.z * (close - ema200).signum() + 0.5 * slope.z - 0.5 * bb.z)
        }
        _ => None,
    };
    ScoreResult {
        value,
        meta: ScoreMeta {
            available: value.is_some(),
            formula: Some(
                "z(ADX14)*sign(close-EMA200)+0.5*z(ema50_slope)-0.5*z(BB_width_pctile)"
                    .to_string(),
            ),
            z: value,
            components: serde_json::json!({
                "adx14": component_json(adx14, adx_stats),
                "ema50_slope": component_json(ema50_slope, slope_stats),
                "bb_width_pctile": component_json(bb_width_pctile, bb_stats),
                "trend_sign": close.zip(ema200).map(|(close, ema200)| (close - ema200).signum()),
            }),
            missing,
            ..ScoreMeta::default()
        },
    }
}

fn evaluate_dpi(state: &SymbolState, n: usize) -> ScoreResult {
    let Some(book) = trusted_full_book(state) else {
        return ScoreResult {
            value: None,
            meta: dpi_meta(n, None, vec!["depth_array_missing".to_string()]),
        };
    };

    let imbalance = book.qty_imbalance_top_n(n);
    let missing = if imbalance.is_some() {
        Vec::new()
    } else {
        vec![format!("bid_depth_lt_{n}"), format!("ask_depth_lt_{n}")]
    };
    ScoreResult {
        value: imbalance.map(|value| value.imbalance),
        meta: dpi_meta(n, imbalance, missing),
    }
}

fn dpi_meta(
    n: usize,
    imbalance: Option<perp_radar_state::book_full::DepthQtyImbalance>,
    missing: Vec<String>,
) -> ScoreMeta {
    ScoreMeta {
        available: imbalance.is_some(),
        formula: Some(format!(
            "(sum_bid_qty_top_{n}-sum_ask_qty_top_{n})/(sum_bid_qty_top_{n}+sum_ask_qty_top_{n})"
        )),
        book_source: Some("full".to_string()),
        components: imbalance
            .map(|value| {
                serde_json::json!({
                    format!("bid_qty_top{n}"): value.bid_qty_top_n,
                    format!("ask_qty_top{n}"): value.ask_qty_top_n,
                    format!("all_qty_top{n}"): value.all_qty_top_n,
                })
            })
            .unwrap_or_else(|| serde_json::json!({})),
        missing,
        ..ScoreMeta::default()
    }
}

fn evaluate_csi(
    state: &SymbolState,
    carry: &CarryBlock,
    price: &PriceBlock,
    config: &ScoreConfig,
) -> ScoreResult {
    let history = &state.score_history;
    let mut missing = Vec::new();
    let funding_abs = carry.funding_z_7d.map(f64::abs);
    let basis_abs = price.basis_bp.map(f64::abs);
    push_missing(&mut missing, "funding_z_7d_missing", funding_abs);
    push_missing(&mut missing, "basis_bp_missing", basis_abs);
    let funding_stats = funding_abs.and_then(|current| {
        robust_component(
            &history.abs_fundz_7d,
            current,
            config.min_samples,
            config.z_clip,
        )
    });
    let basis_stats = basis_abs.and_then(|current| {
        robust_component(
            &history.abs_basis_bp,
            current,
            config.min_samples,
            config.z_clip,
        )
    });
    if missing.is_empty() && (funding_stats.is_none() || basis_stats.is_none()) {
        missing.push("component_window_insufficient".to_string());
    }
    let value = match (funding_stats, basis_stats) {
        (Some(funding), Some(basis)) if missing.is_empty() => Some(funding.z + 0.5 * basis.z),
        _ => None,
    };
    ScoreResult {
        value,
        meta: ScoreMeta {
            available: value.is_some(),
            formula: Some("z(abs(funding_z_7d))+0.5*z(abs(basis_bp))".to_string()),
            z: value,
            components: serde_json::json!({
                "abs_funding_z_7d": component_json(funding_abs, funding_stats),
                "abs_basis_bp": component_json(basis_abs, basis_stats),
            }),
            missing,
            ..ScoreMeta::default()
        },
    }
}

fn evaluate_rpi(
    state: &SymbolState,
    chart: &ChartBlock,
    carry: &CarryBlock,
    price: &PriceBlock,
    liquidity: &LiquidityBlock,
    config: &ScoreConfig,
) -> ScoreResult {
    let history = &state.score_history;
    let mut missing = Vec::new();
    let rsi14 = chart.rsi_14;
    let funding = carry.funding_z_7d;
    let ret_1h = price.ret_1h;
    let i1 = liquidity.i1;
    push_missing(&mut missing, "rsi14_missing", rsi14);
    push_missing(&mut missing, "funding_z_7d_missing", funding);
    push_missing(&mut missing, "ret_1h_missing", ret_1h);
    push_missing(&mut missing, "i1_missing", i1);

    let rsi_extreme = rsi14.map(|value| (value - 50.0).abs());
    let funding_same_side = rsi14
        .zip(funding)
        .map(|(rsi, funding)| ((rsi - 50.0).signum() * funding).max(0.0));
    let book_against_move = ret_1h
        .zip(i1)
        .map(|(ret, i1)| (-ret.signum() * i1).max(0.0));
    let rsi_stats = rsi_extreme.and_then(|current| {
        robust_component(
            &history.rsi_extreme,
            current,
            config.min_samples,
            config.z_clip,
        )
    });
    let funding_stats = funding_same_side.and_then(|current| {
        robust_component(
            &history.funding_same_side,
            current,
            config.min_samples,
            config.z_clip,
        )
    });
    let book_stats = book_against_move.and_then(|current| {
        robust_component(
            &history.book_against_move,
            current,
            config.min_samples,
            config.z_clip,
        )
    });
    if missing.is_empty() && (rsi_stats.is_none() || funding_stats.is_none() || book_stats.is_none())
    {
        missing.push("component_window_insufficient".to_string());
    }
    let value = match (rsi_stats, funding_stats, book_stats) {
        (Some(rsi), Some(funding), Some(book)) if missing.is_empty() => {
            Some(rsi.z + funding.z + book.z)
        }
        _ => None,
    };
    ScoreResult {
        value,
        meta: ScoreMeta {
            available: value.is_some(),
            formula: Some(
                "z(rsi_extreme)+z(funding_same_side)+z(book_against_move)".to_string(),
            ),
            z: value,
            components: serde_json::json!({
                "rsi_extreme": component_json(rsi_extreme, rsi_stats),
                "funding_same_side": component_json(funding_same_side, funding_stats),
                "book_against_move": component_json(book_against_move, book_stats),
            }),
            missing,
            ..ScoreMeta::default()
        },
    }
}

fn evaluate_vov(state: &SymbolState, chart: &ChartBlock, config: &ScoreConfig) -> ScoreResult {
    let history = &state.score_history;
    let mut missing = Vec::new();
    push_missing(&mut missing, "atr_1h_pct_missing", chart.atr_1h_pct);
    push_missing(
        &mut missing,
        "atr_1h_pct_prev_missing",
        chart.atr_1h_pct_prev,
    );
    if matches!(chart.atr_1h_pct_prev, Some(value) if value <= 0.0) {
        missing.push("atr_1h_pct_prev_non_positive".to_string());
    }
    let ratio = chart.atr_1h_pct_delta_ratio;
    let ratio_stats = ratio.and_then(|current| {
        robust_component(
            &history.atr_delta_ratio,
            current,
            config.min_samples,
            config.z_clip,
        )
    });
    if missing.is_empty() && ratio_stats.is_none() {
        missing.push("atr_delta_ratio_window_insufficient".to_string());
    }
    let value = ratio_stats.filter(|_| missing.is_empty()).map(|stats| stats.z);
    ScoreResult {
        value,
        meta: ScoreMeta {
            available: value.is_some(),
            formula: Some("z(atr_delta_ratio)".to_string()),
            z: value,
            components: serde_json::json!({
                "atr_delta_ratio": component_json(ratio, ratio_stats),
            }),
            missing,
            ..ScoreMeta::default()
        },
    }
}

fn push_missing(missing: &mut Vec<String>, reason: &str, value: Option<f64>) {
    if value.filter(|value| value.is_finite()).is_none() {
        missing.push(reason.to_string());
    }
}

#[derive(Debug, Clone, Copy)]
struct ComponentStats {
    n: usize,
    median: f64,
    z: f64,
}

fn robust_component(
    history: &RingWindow,
    current: f64,
    min_samples: usize,
    z_clip: f64,
) -> Option<ComponentStats> {
    let values = history.values_recent();
    if !current.is_finite() || min_samples == 0 || values.len() < min_samples {
        return None;
    }
    let median_value = median(values.clone())?;
    let deviations = values
        .iter()
        .map(|value| (value - median_value).abs())
        .collect::<Vec<_>>();
    let mad = median(deviations)?;
    let mut scale = 1.4826 * mad;
    if scale == 0.0 {
        scale = sample_stddev(&values)?;
    }
    if scale == 0.0 && (current - median_value).abs() == 0.0 {
        return Some(ComponentStats {
            n: values.len(),
            median: median_value,
            z: 0.0,
        });
    }
    if scale == 0.0 || !scale.is_finite() {
        return None;
    }
    let clip = if z_clip.is_finite() && z_clip > 0.0 {
        z_clip
    } else {
        f64::INFINITY
    };
    Some(ComponentStats {
        n: values.len(),
        median: median_value,
        z: ((current - median_value) / scale).clamp(-clip, clip),
    })
}

fn component_json(value: Option<f64>, stats: Option<ComponentStats>) -> serde_json::Value {
    serde_json::json!({
        "value": value,
        "n": stats.map(|stats| stats.n),
        "median": stats.map(|stats| stats.median),
        "z": stats.map(|stats| stats.z),
    })
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        Some((values[mid - 1] + values[mid]) / 2.0)
    } else {
        Some(values[mid])
    }
}

fn sample_stddev(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| {
            let diff = value - mean;
            diff * diff
        })
        .sum::<f64>()
        / (values.len() - 1) as f64;
    Some(variance.sqrt())
}

fn ema_last_from_candles(candles: &[Candle], period: usize) -> Option<f64> {
    if candles.len() < period || period == 0 {
        return None;
    }
    let closes = candles.iter().map(|candle| candle.close).collect::<Vec<_>>();
    ema_last(&closes, period)
}

fn ema_slope_from_candles(candles: &[Candle], period: usize, lookback: usize) -> Option<f64> {
    if lookback == 0 || candles.len() < period + lookback {
        return None;
    }
    let closes = candles.iter().map(|candle| candle.close).collect::<Vec<_>>();
    let now = ema_last(&closes, period)?;
    let past_end = closes.len().checked_sub(lookback)?;
    let past = ema_last(&closes[..past_end], period)?;
    if past == 0.0 {
        return None;
    }
    Some((now - past) / past)
}

fn ema_last(values: &[f64], period: usize) -> Option<f64> {
    if values.len() < period || period == 0 || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let seed = values[..period].iter().sum::<f64>() / period as f64;
    let multiplier = 2.0 / (period as f64 + 1.0);
    Some(
        values[period..]
            .iter()
            .fold(seed, |ema, value| ((value - ema) * multiplier) + ema),
    )
}
