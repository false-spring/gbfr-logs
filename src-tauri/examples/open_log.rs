//! Loads a saved encounter blob the way opening a log in the app does and prints
//! the party rows it produced. `fetch_encounter_state` is a Tauri command and
//! cannot be called outside the app, but it starts with the same
//! `deserialize_version` + `reparse`, so a panic here is a panic there.
//!
//! cargo run --example open_log -- <blob.bin> [version]

use gbfr_logs::parser;

fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: open_log <blob.bin> [version]"))?;
    let version: u8 = std::env::args().nth(2).unwrap_or_default().parse().unwrap_or(1);
    let blob = std::fs::read(&path)?;

    let mut parser = parser::deserialize_version(&blob, version)
        .map_err(|e| anyhow::anyhow!("deserialize failed: {e}"))?;
    println!("deserialized ok");

    parser.reparse();
    println!("reparse ok");

    let derived = &parser.derived_state;
    println!("\nduration {} ms", derived.duration());
    println!("party rows: {}", derived.party.len());
    for (index, row) in derived.party.iter() {
        println!("  {index:#010x}: {:?}", row.character_type);
    }

    let json = serde_json::to_string(&derived).map_err(|e| anyhow::anyhow!("serialize: {e}"))?;
    println!("\nderived state serializes to {} bytes of JSON", json.len());

    Ok(())
}
