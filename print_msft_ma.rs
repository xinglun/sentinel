use std::fs;
use serde_json::Value;

fn main() {
    let raw = fs::read_to_string("reports/2026-02-24.json").unwrap();
    let parsed: Vec<Value> = serde_json::from_str(&raw).unwrap();
    for v in parsed {
        if v["symbol"] == "MSFT" {
            println!("MSFT price: {}", v["dog_price"]);
            println!("MSFT leash_ma: {}", v["leash_ma"]);
            // NOTE: caution_ma_val isn't explicitly in the JSON right now, 
            // but we know it's failing the reference_price >= caution_ma_val check 
            // OR the trend != Down check in engine.rs
        }
    }
}
