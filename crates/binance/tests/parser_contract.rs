use perp_radar_binance::parser::{parse_combined_event, BinanceEvent};

#[test]
fn parses_combined_kline_event() {
    let payload = r#"{
      "stream":"btcusdt@kline_1m",
      "data":{
        "e":"kline",
        "E":1714521600000,
        "s":"BTCUSDT",
        "k":{
          "t":1714521600000,
          "T":1714521659999,
          "s":"BTCUSDT",
          "i":"1m",
          "o":"64000.0",
          "c":"64100.0",
          "h":"64200.0",
          "l":"63950.0",
          "v":"12.5",
          "q":"801250.0",
          "n":120,
          "V":"6.0",
          "Q":"384600.0",
          "x":true
        }
      }
    }"#;

    let event = parse_combined_event(payload).unwrap();
    match event {
        BinanceEvent::Kline(kline) => {
            assert_eq!(kline.stream, "btcusdt@kline_1m");
            assert_eq!(kline.update.candle.symbol, "BTCUSDT");
            assert!(kline.update.candle.is_closed);
            assert_eq!(kline.update.candle.close, 64100.0);
        }
        other => panic!("expected kline event, got {other:?}"),
    }
}

#[test]
fn parses_depth_update_sequence_fields() {
    let payload = r#"{
      "stream":"btcusdt@depth@500ms",
      "data":{
        "e":"depthUpdate",
        "E":1714521600000,
        "T":1714521600000,
        "s":"BTCUSDT",
        "U":101,
        "u":110,
        "pu":100,
        "b":[["64000.0","1.2"]],
        "a":[["64001.0","0.8"]]
      }
    }"#;

    let event = parse_combined_event(payload).unwrap();
    match event {
        BinanceEvent::Depth(delta) => {
            assert_eq!(delta.stream, "btcusdt@depth@500ms");
            assert_eq!(delta.symbol, "BTCUSDT");
            assert_eq!(delta.first_update_id, 101);
            assert_eq!(delta.previous_final_update_id, 100);
        }
        other => panic!("expected depth event, got {other:?}"),
    }
}

#[test]
fn parses_partial_depth_event() {
    let payload = r#"{
      "stream":"btcusdt@depth20@500ms",
      "data":{
        "lastUpdateId":110,
        "E":1714521600000,
        "T":1714521600000,
        "bids":[["64000.0","1.2"]],
        "asks":[["64001.0","0.8"]]
      }
    }"#;

    let event = parse_combined_event(payload).unwrap();
    match event {
        BinanceEvent::PartialDepth(partial) => {
            assert_eq!(partial.stream, "btcusdt@depth20@500ms");
            assert_eq!(partial.symbol, "BTCUSDT");
            assert_eq!(partial.last_update_id, 110);
            assert_eq!(partial.bids[0].price, 64000.0);
            assert_eq!(partial.asks[0].qty, 0.8);
        }
        other => panic!("expected partial depth event, got {other:?}"),
    }
}

#[test]
fn parses_exact_depth20_500ms_partial_depth_stream() {
    let payload = r#"{
      "stream":"btcusdt@depth20@500ms",
      "data":{
        "lastUpdateId":110,
        "E":1714521600000,
        "T":1714521600000,
        "bids":[["64000.0","1.2"]],
        "asks":[["64001.0","0.8"]]
      }
    }"#;

    let event = parse_combined_event(payload).unwrap();
    assert!(matches!(event, BinanceEvent::PartialDepth(_)));
}

#[test]
fn ignores_malformed_partial_depth_interval() {
    let payload = r#"{
      "stream":"btcusdt@depth20@bad",
      "data":{
        "lastUpdateId":110,
        "E":1714521600000,
        "T":1714521600000,
        "bids":[["64000.0","1.2"]],
        "asks":[["64001.0","0.8"]]
      }
    }"#;

    let event = parse_combined_event(payload).unwrap();
    assert!(matches!(event, BinanceEvent::Ignored));
}

#[test]
fn rejects_partial_depth_stream_with_empty_symbol() {
    let payload = r#"{
      "stream":"@depth20@500ms",
      "data":{
        "lastUpdateId":110,
        "E":1714521600000,
        "T":1714521600000,
        "bids":[["64000.0","1.2"]],
        "asks":[["64001.0","0.8"]]
      }
    }"#;

    let err = parse_combined_event(payload).unwrap_err();
    assert!(err.to_string().contains("missing symbol"));
}

#[test]
fn rejects_kline_non_finite_and_negative_numbers() {
    for (field, value) in [
        ("o", "NaN"),
        ("c", "inf"),
        ("h", "-1.0"),
        ("l", "0.0"),
        ("v", "-0.1"),
        ("q", "-0.1"),
        ("V", "-0.1"),
        ("Q", "-0.1"),
    ] {
        let payload = kline_payload_with(field, value);
        let err = parse_combined_event(&payload).unwrap_err();
        assert!(
            err.to_string().contains(field),
            "expected error for {field} to mention field, got {err:?}"
        );
    }
}

#[test]
fn rejects_depth_non_finite_and_negative_levels_but_allows_delete_qty() {
    for (side, field, value) in [
        ("b", "price", "NaN"),
        ("b", "price", "inf"),
        ("b", "price", "-1.0"),
        ("b", "qty", "NaN"),
        ("b", "qty", "inf"),
        ("b", "qty", "-1.0"),
        ("a", "price", "NaN"),
        ("a", "price", "inf"),
        ("a", "price", "-1.0"),
        ("a", "qty", "NaN"),
        ("a", "qty", "inf"),
        ("a", "qty", "-1.0"),
    ] {
        let payload = depth_payload_with(side, field, value);
        let err = parse_combined_event(&payload).unwrap_err();
        assert!(
            err.to_string().contains("[0]"),
            "expected error for {side} {field} to mention level index, got {err:?}"
        );
    }

    let payload = r#"{
      "stream":"btcusdt@depth@500ms",
      "data":{
        "e":"depthUpdate",
        "E":1714521600000,
        "T":1714521600000,
        "s":"BTCUSDT",
        "U":101,
        "u":110,
        "pu":100,
        "b":[["64000.0","0.0"]],
        "a":[["64001.0","0.0"]]
      }
    }"#;

    let event = parse_combined_event(payload).unwrap();
    match event {
        BinanceEvent::Depth(delta) => {
            assert_eq!(delta.bids[0].qty, 0.0);
            assert_eq!(delta.asks[0].qty, 0.0);
        }
        other => panic!("expected depth event, got {other:?}"),
    }
}

#[test]
fn rejects_partial_depth_non_finite_and_negative_levels() {
    for (side, field, value) in [
        ("bids", "price", "NaN"),
        ("bids", "price", "inf"),
        ("bids", "price", "-1.0"),
        ("bids", "qty", "NaN"),
        ("bids", "qty", "inf"),
        ("bids", "qty", "-1.0"),
        ("asks", "price", "NaN"),
        ("asks", "price", "inf"),
        ("asks", "price", "-1.0"),
        ("asks", "qty", "NaN"),
        ("asks", "qty", "inf"),
        ("asks", "qty", "-1.0"),
    ] {
        let payload = partial_depth_payload_with(side, field, value);
        let err = parse_combined_event(&payload).unwrap_err();
        assert!(
            err.to_string().contains("[0]"),
            "expected error for {side} {field} to mention level index, got {err:?}"
        );
    }
}

#[test]
fn rejects_stream_symbol_mismatch() {
    for payload in [
        r#"{
          "stream":"ethusdt@kline_1m",
          "data":{
            "e":"kline",
            "E":1714521600000,
            "s":"BTCUSDT",
            "k":{
              "t":1714521600000,
              "T":1714521659999,
              "s":"BTCUSDT",
              "i":"1m",
              "o":"64000.0",
              "c":"64100.0",
              "h":"64200.0",
              "l":"63950.0",
              "v":"12.5",
              "q":"801250.0",
              "n":120,
              "V":"6.0",
              "Q":"384600.0",
              "x":true
            }
          }
        }"#,
        r#"{
          "stream":"ethusdt@depth@500ms",
          "data":{
            "e":"depthUpdate",
            "E":1714521600000,
            "T":1714521600000,
            "s":"BTCUSDT",
            "U":101,
            "u":110,
            "pu":100,
            "b":[["64000.0","1.2"]],
            "a":[["64001.0","0.8"]]
          }
        }"#,
        r#"{
          "stream":"ethusdt@depth20@500ms",
          "data":{
            "s":"BTCUSDT",
            "lastUpdateId":110,
            "E":1714521600000,
            "T":1714521600000,
            "bids":[["64000.0","1.2"]],
            "asks":[["64001.0","0.8"]]
          }
        }"#,
    ] {
        let err = parse_combined_event(payload).unwrap_err();
        assert!(err.to_string().contains("stream symbol"));
    }
}

fn kline_payload_with(field: &str, value: &str) -> String {
    let mut fields = [
        ("o", "64000.0"),
        ("c", "64100.0"),
        ("h", "64200.0"),
        ("l", "63950.0"),
        ("v", "12.5"),
        ("q", "801250.0"),
        ("V", "6.0"),
        ("Q", "384600.0"),
    ];
    for (key, field_value) in &mut fields {
        if *key == field {
            *field_value = value;
        }
    }

    format!(
        r#"{{
          "stream":"btcusdt@kline_1m",
          "data":{{
            "e":"kline",
            "E":1714521600000,
            "s":"BTCUSDT",
            "k":{{
              "t":1714521600000,
              "T":1714521659999,
              "s":"BTCUSDT",
              "i":"1m",
              "o":"{}",
              "c":"{}",
              "h":"{}",
              "l":"{}",
              "v":"{}",
              "q":"{}",
              "n":120,
              "V":"{}",
              "Q":"{}",
              "x":true
            }}
          }}
        }}"#,
        fields[0].1,
        fields[1].1,
        fields[2].1,
        fields[3].1,
        fields[4].1,
        fields[5].1,
        fields[6].1,
        fields[7].1
    )
}

fn depth_payload_with(side: &str, field: &str, value: &str) -> String {
    let bid_price = if side == "b" && field == "price" {
        value
    } else {
        "64000.0"
    };
    let bid_qty = if side == "b" && field == "qty" {
        value
    } else {
        "1.2"
    };
    let ask_price = if side == "a" && field == "price" {
        value
    } else {
        "64001.0"
    };
    let ask_qty = if side == "a" && field == "qty" {
        value
    } else {
        "0.8"
    };

    format!(
        r#"{{
          "stream":"btcusdt@depth@500ms",
          "data":{{
            "e":"depthUpdate",
            "E":1714521600000,
            "T":1714521600000,
            "s":"BTCUSDT",
            "U":101,
            "u":110,
            "pu":100,
            "b":[["{bid_price}","{bid_qty}"]],
            "a":[["{ask_price}","{ask_qty}"]]
          }}
        }}"#
    )
}

fn partial_depth_payload_with(side: &str, field: &str, value: &str) -> String {
    let bid_price = if side == "bids" && field == "price" {
        value
    } else {
        "64000.0"
    };
    let bid_qty = if side == "bids" && field == "qty" {
        value
    } else {
        "1.2"
    };
    let ask_price = if side == "asks" && field == "price" {
        value
    } else {
        "64001.0"
    };
    let ask_qty = if side == "asks" && field == "qty" {
        value
    } else {
        "0.8"
    };

    format!(
        r#"{{
          "stream":"btcusdt@depth20@500ms",
          "data":{{
            "s":"BTCUSDT",
            "lastUpdateId":110,
            "E":1714521600000,
            "T":1714521600000,
            "bids":[["{bid_price}","{bid_qty}"]],
            "asks":[["{ask_price}","{ask_qty}"]]
          }}
        }}"#
    )
}
