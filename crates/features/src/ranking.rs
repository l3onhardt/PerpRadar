#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub symbol: String,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankedCandidate {
    pub symbol: String,
    pub score: f64,
    pub rank: usize,
}

pub fn rank_candidates(mut candidates: Vec<Candidate>) -> Vec<RankedCandidate> {
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
