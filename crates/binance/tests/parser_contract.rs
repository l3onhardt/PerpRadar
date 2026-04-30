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
            assert_eq!(kline.candle.symbol, "BTCUSDT");
            assert!(kline.candle.is_closed);
            assert_eq!(kline.candle.close, 64100.0);
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
            assert_eq!(delta.symbol, "BTCUSDT");
            assert_eq!(delta.first_update_id, 101);
            assert_eq!(delta.previous_final_update_id, 100);
        }
        other => panic!("expected depth event, got {other:?}"),
    }
}
