use perp_radar_state::book_full::FullBook;
use perp_radar_state::book_partial::BookLevel;
use perp_radar_state::score_history::{RingWindow, ScoreHistoryState};

#[test]
fn ring_window_keeps_only_finite_bounded_values() {
    let mut window = RingWindow::new(3);
    window.push(1.0);
    window.push(f64::NAN);
    window.push(2.0);
    window.push(3.0);
    window.push(4.0);

    assert_eq!(window.len(), 3);
    assert_eq!(window.values_recent(), vec![2.0, 3.0, 4.0]);
}

#[test]
fn score_history_does_not_record_lri_book_components_when_book_is_untrusted() {
    let book = FullBook::from_snapshot(
        "BTCUSDT",
        10,
        vec![
            BookLevel {
                price: 100.0,
                qty: 100.0,
            },
            BookLevel {
                price: 99.99,
                qty: 100.0,
            },
        ],
        vec![
            BookLevel {
                price: 100.01,
                qty: 100.0,
            },
            BookLevel {
                price: 100.02,
                qty: 100.0,
            },
        ],
    );
    let mut history = ScoreHistoryState::new(120);

    history.record_lri_book_components(Some(&book), false, 1_000.0);

    assert_eq!(history.neg_spread_bp.len(), 0);
    assert_eq!(history.liq_5bp_usd.len(), 0);
    assert_eq!(history.neg_slip_bp.len(), 0);
}

#[test]
fn score_history_records_lri_book_components_when_full_book_is_trusted() {
    let book = FullBook::from_snapshot(
        "BTCUSDT",
        10,
        vec![
            BookLevel {
                price: 100.0,
                qty: 100.0,
            },
            BookLevel {
                price: 99.99,
                qty: 100.0,
            },
        ],
        vec![
            BookLevel {
                price: 100.01,
                qty: 100.0,
            },
            BookLevel {
                price: 100.02,
                qty: 100.0,
            },
        ],
    );
    let mut history = ScoreHistoryState::new(120);

    history.record_lri_book_components(Some(&book), true, 1_000.0);

    assert_eq!(history.neg_spread_bp.len(), 1);
    assert_eq!(history.liq_5bp_usd.len(), 1);
    assert_eq!(history.neg_slip_bp.len(), 1);
    assert!(history.latest_neg_spread_bp.unwrap() < 0.0);
    assert!(history.latest_liq_5bp_usd.unwrap() > 0.0);
    assert!(history.latest_neg_slip_bp.unwrap() < 0.0);
}
