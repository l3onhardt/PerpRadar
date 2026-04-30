use perp_radar_state::book_full::{BookDelta, FullBook, LevelDelta};
use perp_radar_state::book_partial::{BookLevel, PartialBook};

#[test]
fn partial_book_calculates_spread_imbalance_and_microprice() {
    let book = PartialBook::new(
        "BTCUSDT",
        vec![
            BookLevel {
                price: 100.0,
                qty: 10.0,
            },
            BookLevel {
                price: 99.9,
                qty: 5.0,
            },
        ],
        vec![
            BookLevel {
                price: 100.1,
                qty: 6.0,
            },
            BookLevel {
                price: 100.2,
                qty: 4.0,
            },
        ],
    );

    assert!((book.spread_bp().unwrap() - 10.0).abs() < 0.0001);
    assert!((book.imbalance_top_n(1).unwrap() - 0.25).abs() < 0.0001);
    assert!(book.microprice_bp().unwrap() > 0.0);
}

#[test]
fn full_book_rejects_sequence_gap() {
    let mut book = FullBook::from_snapshot(
        "BTCUSDT",
        10,
        vec![BookLevel {
            price: 100.0,
            qty: 10.0,
        }],
        vec![BookLevel {
            price: 100.1,
            qty: 6.0,
        }],
    );

    let result = book.apply_delta(BookDelta {
        first_update_id: 12,
        final_update_id: 13,
        previous_final_update_id: 9,
        bids: vec![LevelDelta {
            price: 100.0,
            qty: 11.0,
        }],
        asks: vec![],
    });

    assert!(result.is_err());
    assert!(!book.seq_ok());
}
