use chrono::Utc;
use perp_radar_core::packet::{
    CarryBlock, ChartBlock, EventsBlock, LiquidityBlock, PacketProfile, PriceBlock, ScoresBlock,
    StandardPacket, UniverseBlock,
};
use perp_radar_core::quality::QualityReason;
use perp_radar_core::types::{Candle, UniverseTier};
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
    let price = PriceBlock {
        last: last_price,
        mark: state.mark_price,
        index: state.index_price,
        basis_bp: basis_bp(state.mark_price, state.index_price),
        ret_1m: tail_return(&candles, 1),
        ret_5m: tail_return(&candles, 5),
        ret_15m: tail_return(&candles, 15),
        ret_1h: tail_return(&candles, 60),
    };

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
        packet_schema: "2.0".to_string(),
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
        chart: ChartBlock {
            signature: chart_signature(&candles),
            regime: technicals
                .as_ref()
                .and_then(|snapshot| snapshot.regime.clone()),
            ema_20: technicals.as_ref().and_then(|snapshot| snapshot.ema_20),
            ema_50: technicals.as_ref().and_then(|snapshot| snapshot.ema_50),
            rsi_14: technicals.as_ref().and_then(|snapshot| snapshot.rsi_14),
            macd_histogram: technicals
                .as_ref()
                .and_then(|snapshot| snapshot.macd_histogram),
            atr_pct: technicals.as_ref().and_then(|snapshot| snapshot.atr_pct),
            bb_width: technicals.as_ref().and_then(|snapshot| snapshot.bb_width),
            adx_14: technicals.as_ref().and_then(|snapshot| snapshot.adx_14),
            vwap_20: technicals.as_ref().and_then(|snapshot| snapshot.vwap_20),
            cmf_20: technicals.as_ref().and_then(|snapshot| snapshot.cmf_20),
        },
        liquidity: LiquidityBlock {
            book_mode: state.quality.book_mode.clone(),
            spread_bp: state
                .partial_book
                .as_ref()
                .and_then(|book| book.spread_bp()),
            i1: state
                .partial_book
                .as_ref()
                .and_then(|book| book.imbalance_top_n(1)),
            i5: state
                .partial_book
                .as_ref()
                .and_then(|book| book.imbalance_top_n(5)),
            microprice_bp: state
                .partial_book
                .as_ref()
                .and_then(|book| book.microprice_bp()),
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
        },
        carry: CarryBlock {
            funding_now: state.funding_rate,
            funding_unit: state
                .funding_rate
                .map(|_| format!("{funding_interval_hours}h_estimate")),
            funding_interval_hours: state.funding_rate.map(|_| funding_interval_hours),
            funding_z_7d: state
                .funding_rate
                .and_then(|rate| crate::funding::z_score(&state.funding_history, rate)),
            next_funding_time: state.next_funding_time,
        },
        events: EventsBlock {
            liq_1m_usd: liquidation_total(state, 60_000),
            liq_5m_usd: liquidation_total(state, 300_000),
            liq_15m_usd: liquidation_total(state, 900_000),
            liq_side: dominant_liquidation_side(state),
            volume_spike_z: volume_spike_z(&candles),
        },
        scores: scores_block(state, &candles),
        quality,
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

fn scores_block(state: &SymbolState, candles: &[Candle]) -> ScoresBlock {
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
    let liq_score = liquidation_total(state, 300_000).map(|value| (value / 1_000_000.0).min(3.0));
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
        liquidation_event_score: liq_score,
        squeeze_or_breakout_score: squeeze_or_breakout.or(Some(0.0)),
        liquidity_quality,
    });

    ScoresBlock {
        tcs,
        lri: liq_score,
        dpi5: state
            .partial_book
            .as_ref()
            .and_then(|book| book.imbalance_top_n(5)),
        csi: squeeze_or_breakout,
        rpi: ret_15m_abs,
        vov: volume_z,
    }
}
