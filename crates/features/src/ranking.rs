#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub symbol: String,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UniverseRankingInput {
    pub symbol: String,
    pub quote_volume_24h: Option<f64>,
    pub price_change_percent_24h: Option<f64>,
    pub funding_rate: Option<f64>,
    pub liquidation_5m_usd: Option<f64>,
    pub ret_15m: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankedCandidate {
    pub symbol: String,
    pub score: f64,
    pub rank: usize,
}

pub fn rank_candidates(mut candidates: Vec<Candidate>) -> Vec<RankedCandidate> {
    candidates.retain(|candidate| candidate.score.is_finite());
    candidates.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.symbol.cmp(&b.symbol))
    });
    candidates
        .into_iter()
        .enumerate()
        .map(|(idx, candidate)| RankedCandidate {
            symbol: candidate.symbol,
            score: candidate.score,
            rank: idx + 1,
        })
        .collect()
}

pub fn rank_u0_universe(
    inputs: Vec<UniverseRankingInput>,
    limit: usize,
) -> Vec<RankedCandidate> {
    let candidates = inputs
        .into_iter()
        .filter_map(|input| {
            let score = u0_score(&input)?;
            Some(Candidate {
                symbol: input.symbol,
                score,
            })
        })
        .collect::<Vec<_>>();

    let mut ranked = rank_candidates(candidates);
    ranked.truncate(limit);
    ranked
}

fn u0_score(input: &UniverseRankingInput) -> Option<f64> {
    let quote_volume = required_non_negative(input.quote_volume_24h)?;
    let price_change = required_finite(input.price_change_percent_24h)?.abs();
    let funding_stress = input
        .funding_rate
        .filter(|value| value.is_finite())
        .map(|value| (value.abs() / 0.0001).min(6.0))
        .unwrap_or(0.0);
    let liquidation = input
        .liquidation_5m_usd
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(0.0);
    let momentum = input
        .ret_15m
        .filter(|value| value.is_finite())
        .map(|value| (value.abs() * 100.0).min(6.0))
        .unwrap_or(0.0);

    let volume_score = (quote_volume.ln_1p() / 20.0).clamp(0.0, 2.0);
    let price_score = (price_change / 5.0).clamp(0.0, 2.0);
    let liquidation_score = (liquidation.ln_1p() / 14.0).clamp(0.0, 2.0);

    let score = 0.45 * volume_score
        + 0.20 * price_score
        + 0.15 * funding_stress
        + 0.15 * liquidation_score
        + 0.05 * momentum;

    score.is_finite().then_some(score)
}

fn required_non_negative(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value >= 0.0)
}

fn required_finite(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite())
}
