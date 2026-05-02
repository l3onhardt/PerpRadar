use perp_radar_core::packet::StandardPacket;

fn fmt_opt<T: std::fmt::Display>(value: Option<T>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

pub fn packet_to_text(packet: &StandardPacket) -> String {
    format!(
        "[{symbol}] rank:{rank} tier:{tier:?} price:last={last},mark={mark},basis_bp={basis} \
chart:regime={regime},rsi14={rsi},atr_pct={atr},bb_width={bb},adx14={adx},macd_hist={macd} \
liquidity:book={book},spread_bp={spread},liq_5bp_usd={liq5},liq_10bp_usd={liq10} \
carry:funding_now={funding},unit={funding_unit},z_7d={funding_z} \
events:liq_1m_usd={liq1},liq_5m_usd={liq5m},liq_15m_usd={liq15m},side={liq_side},volume_spike_z={volume_spike} \
scores:TCS={tcs},LRI={lri},DPI5={dpi5},DPI10={dpi10},CSI={csi},RPI={rpi},VoV={vov} \
quality:warm={warm},stale={stale},freshness_ms={freshness},reasons={reasons:?}",
        symbol = packet.symbol,
        rank = packet.rank,
        tier = packet.universe.tier,
        last = fmt_opt(packet.price.last),
        mark = fmt_opt(packet.price.mark),
        basis = fmt_opt(packet.price.basis_bp),
        regime = packet.chart.regime.as_deref().unwrap_or("null"),
        rsi = fmt_opt(packet.chart.rsi_14),
        atr = fmt_opt(packet.chart.atr_pct),
        bb = fmt_opt(packet.chart.bb_width),
        adx = fmt_opt(packet.chart.adx_14),
        macd = fmt_opt(packet.chart.macd_histogram),
        book = packet.liquidity.book_mode,
        spread = fmt_opt(packet.liquidity.spread_bp),
        liq5 = fmt_opt(packet.liquidity.liq_5bp_usd),
        liq10 = fmt_opt(packet.liquidity.liq_10bp_usd),
        funding = fmt_opt(packet.carry.funding_now),
        funding_unit = packet.carry.funding_unit.as_deref().unwrap_or("null"),
        funding_z = fmt_opt(packet.carry.funding_z_7d),
        liq1 = fmt_opt(packet.events.liq_1m_usd),
        liq5m = fmt_opt(packet.events.liq_5m_usd),
        liq15m = fmt_opt(packet.events.liq_15m_usd),
        liq_side = packet.events.liq_side.as_deref().unwrap_or("null"),
        volume_spike = fmt_opt(packet.events.volume_spike_z),
        tcs = fmt_opt(packet.scores.tcs),
        lri = fmt_opt(packet.scores.lri),
        dpi5 = fmt_opt(packet.scores.dpi5),
        dpi10 = fmt_opt(packet.scores.dpi10),
        csi = fmt_opt(packet.scores.csi),
        rpi = fmt_opt(packet.scores.rpi),
        vov = fmt_opt(packet.scores.vov),
        warm = packet.quality.warm,
        stale = packet.quality.stale,
        freshness = packet.quality.freshness_ms,
        reasons = packet.quality.reasons,
    )
}
