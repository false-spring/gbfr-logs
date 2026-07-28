//! Runs the shipped SBA gauge-attribution rules over a saved encounter blob and
//! prints what they decided as JSON, keyed by index into the raw event log.
//! `blob.bin` is the raw `logs.data` column.
//!
//! cargo run --example sba_infer -- <blob.bin> [--move-ms N] [--counter-ms N] [--learning-ms N] [--value-ms N]

use std::collections::BTreeMap;

use gbfr_logs::parser::v1::{
    sba_inference::{infer_remote_causes_tagged, Rule, Windows},
    Encounter,
};

fn rule_name(rule: Rule) -> &'static str {
    match rule {
        Rule::Counter => "0-counter",
        Rule::Redistribute => "1-redistribute",
        Rule::PartyFill => "1a-party-fill",
        Rule::FlatGrant => "2-flat-grant",
        Rule::Move => "3-move",
        Rule::MoveByValue => "3c-move-by-value",
        Rule::RetroCounter => "3b-retro-counter",
        Rule::DamageTaken => "4-damage-taken",
        Rule::DamageTakenByValue => "4b-damage-taken-value",
        Rule::CappedGrant => "5-capped-grant",
        Rule::SummonCall => "6-summon-call",
        Rule::FlatGrantFallback => "7-flat-grant-fallback",
    }
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let mut path: Option<String> = None;
    let mut windows = Windows::default();

    while let Some(arg) = args.next() {
        let mut value = || -> anyhow::Result<i64> {
            args.next()
                .ok_or_else(|| anyhow::anyhow!("{arg} needs a value"))?
                .parse()
                .map_err(Into::into)
        };
        match arg.as_str() {
            "--move-ms" => windows.move_ms = value()?,
            "--counter-ms" => windows.counter_ms = value()?,
            "--learning-ms" => windows.learning_ms = value()?,
            "--value-ms" => windows.value_ms = value()?,
            _ if path.is_none() => path = Some(arg),
            _ => anyhow::bail!("unexpected argument {arg}"),
        }
    }

    let path = path.ok_or_else(|| anyhow::anyhow!("usage: sba_infer <blob.bin> [--move-ms N]"))?;
    let mut encounter = Encounter::from_blob(&std::fs::read(&path)?)?;
    encounter.repopulate_event_log();

    let decided: BTreeMap<usize, serde_json::Value> =
        infer_remote_causes_tagged(&encounter, windows)
            .into_iter()
            .map(|(index, (cause, rule))| {
                (
                    index,
                    serde_json::json!({
                        "cause": cause,
                        "rule": rule_name(rule),
                    }),
                )
            })
            .collect();

    println!(
        "{}",
        serde_json::json!({
            "windows": {
                "move_ms": windows.move_ms,
                "counter_ms": windows.counter_ms,
                "learning_ms": windows.learning_ms,
                "value_ms": windows.value_ms,
            },
            "events": encounter.event_log().count(),
            "decided": decided,
        })
    );

    Ok(())
}
