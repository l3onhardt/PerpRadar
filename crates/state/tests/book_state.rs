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

    let expected_spread_bp = (100.1 - 100.0) / ((100.0 + 100.1) / 2.0) * 10_000.0;
    let expected_imbalance = ((100.0 * 10.0) - (100.1 * 6.0)) / ((100.0 * 10.0) + (100.1 * 6.0));

    assert!((book.spread_bp().unwrap() - expected_spread_bp).abs() < 0.0001);
    assert!((book.imbalance_top_n(1).unwrap() - expected_imbalance).abs() < 0.0001);
    assert!(book.microprice_bp().unwrap() > 0.0);
}

#[test]
fn partial_book_microprice_is_none_when_top_quantities_are_zero() {
    let book = PartialBook::new(
        "BTCUSDT",
        vec![BookLevel {
            price: 100.0,
            qty: 0.0,
        }],
        vec![BookLevel {
            price: 100.1,
            qty: 0.0,
        }],
    );

    assert_eq!(book.microprice_bp(), None);
}

#[test]
fn full_book_accepts_first_delta_that_covers_snapshot() {
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
        first_update_id: 8,
        final_update_id: 11,
        previous_final_update_id: 7,
        bids: vec![LevelDelta {
            price: 100.0,
            qty: 11.0,
        }],
        asks: vec![],
    });

    assert!(result.is_ok());
    assert!(book.seq_ok());
}

#[test]
fn full_book_ignores_delta_that_is_older_than_snapshot() {
    let mut book = FullBook::from_snapshot(
        "BTCUSDT",
        10,
        vec![BookLevel {
            price: 100.0,
            qty: 10.0,
        }],
        vec![BookLevel {
            price: 100.1,
            qty: 5.0,
        }],
    );

    book.apply_delta(BookDelta {
        first_update_id: 1,
        final_update_id: 9,
        previous_final_update_id: 0,
        bids: vec![LevelDelta {
            price: 100.0,
            qty: 99.0,
        }],
        asks: vec![],
    })
    .unwrap();

    assert!(book.seq_ok());
    assert!(book.visible_liquidity_usd(5.0).unwrap() < 2_000.0);
}

#[test]
fn full_book_rejects_steady_state_gap_after_bootstrap() {
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

    let bootstrap_result = book.apply_delta(BookDelta {
        first_update_id: 8,
        final_update_id: 11,
        previous_final_update_id: 7,
        bids: vec![LevelDelta {
            price: 100.0,
            qty: 11.0,
        }],
        asks: vec![],
    });

    let steady_state_result = book.apply_delta(BookDelta {
        first_update_id: 12,
        final_update_id: 13,
        previous_final_update_id: 10,
        bids: vec![LevelDelta {
            price: 100.0,
            qty: 12.0,
        }],
        asks: vec![],
    });

    assert!(bootstrap_result.is_ok());
    assert!(steady_state_result.is_err());
    assert!(!book.seq_ok());
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

#[test]
fn full_book_can_recover_after_sequence_gap_with_new_snapshot() {
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

    assert!(book
        .apply_delta(BookDelta {
            first_update_id: 12,
            final_update_id: 13,
            previous_final_update_id: 9,
            bids: vec![],
            asks: vec![],
        })
        .is_err());
    assert!(!book.seq_ok());

    book.reset_from_snapshot(
        20,
        vec![BookLevel {
            price: 101.0,
            qty: 4.0,
        }],
        vec![BookLevel {
            price: 101.1,
            qty: 5.0,
        }],
    );
    assert!(book
        .apply_delta(BookDelta {
            first_update_id: 18,
            final_update_id: 21,
            previous_final_update_id: 17,
            bids: vec![LevelDelta {
                price: 101.0,
                qty: 6.0,
            }],
            asks: vec![],
        })
        .is_ok());

    assert!(book.seq_ok());
    assert!(book.visible_liquidity_usd(5.0).unwrap() > 1_000.0);
}

#[test]
fn full_book_calculates_visible_liquidity_and_slippage() {
    let book = FullBook::from_snapshot(
        "BTCUSDT",
        10,
        vec![
            BookLevel {
                price: 100.0,
                qty: 10.0,
            },
            BookLevel {
                price: 99.96,
                qty: 5.0,
            },
            BookLevel {
                price: 99.90,
                qty: 20.0,
            },
        ],
        vec![
            BookLevel {
                price: 100.1,
                qty: 6.0,
            },
            BookLevel {
                price: 100.14,
                qty: 8.0,
            },
            BookLevel {
                price: 100.20,
                qty: 12.0,
            },
        ],
    );

    assert_eq!(book.best_bid(), Some(100.0));
    assert_eq!(book.best_ask(), Some(100.1));
    assert_eq!(
        book.visible_liquidity_usd(5.0).unwrap(),
        100.0 * 10.0 + 100.1 * 6.0
    );
    assert_eq!(
        book.visible_liquidity_usd(10.0).unwrap(),
        100.0 * 10.0 + 99.96 * 5.0 + 100.1 * 6.0 + 100.14 * 8.0
    );
    assert!(book.slippage_bp_for_notional(1_000.0, true).unwrap() > 0.0);
    assert!(book.slippage_bp_for_notional(1_000.0, false).unwrap() > 0.0);
}

#[test]
fn full_book_calculates_spread_bp_from_top_of_book() {
    let book = FullBook::from_snapshot(
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

    let expected = (100.1 - 100.0) / 100.05 * 10_000.0;

    assert!((book.spread_bp().unwrap() - expected).abs() < 0.0001);
}

#[test]
fn full_book_calculates_qty_imbalance_top_n_for_dpi() {
    let book = FullBook::from_snapshot(
        "BTCUSDT",
        10,
        vec![
            BookLevel {
                price: 100.0,
                qty: 10.0,
            },
            BookLevel {
                price: 99.9,
                qty: 5.0,
            },
            BookLevel {
                price: 99.8,
                qty: 1.0,
            },
        ],
        vec![
            BookLevel {
                price: 100.1,
                qty: 6.0,
            },
            BookLevel {
                price: 100.2,
                qty: 2.0,
            },
            BookLevel {
                price: 100.3,
                qty: 2.0,
            },
        ],
    );

    let dpi2 = book.qty_imbalance_top_n(2).unwrap();

    assert_eq!(dpi2.bid_qty_top_n, 15.0);
    assert_eq!(dpi2.ask_qty_top_n, 8.0);
    assert_eq!(dpi2.all_qty_top_n, 23.0);
    assert!((dpi2.imbalance - ((15.0 - 8.0) / 23.0)).abs() < 0.0001);
}

#[test]
fn full_book_qty_imbalance_is_none_when_depth_is_insufficient_or_zero() {
    let shallow = FullBook::from_snapshot(
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
    let zero = FullBook::from_snapshot(
        "BTCUSDT",
        10,
        vec![
            BookLevel {
                price: 100.0,
                qty: 0.0,
            },
            BookLevel {
                price: 99.9,
                qty: 0.0,
            },
        ],
        vec![
            BookLevel {
                price: 100.1,
                qty: 0.0,
            },
            BookLevel {
                price: 100.2,
                qty: 0.0,
            },
        ],
    );

    assert_eq!(shallow.qty_imbalance_top_n(2), None);
    assert_eq!(zero.qty_imbalance_top_n(2), None);
}
