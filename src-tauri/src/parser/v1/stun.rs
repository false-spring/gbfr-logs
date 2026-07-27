use std::collections::HashMap;

use protocol::DamageEvent;

/// Rebuilds each enemy's break gauge from per-hit stun, so counted stun caps when the gauge saturates.
#[derive(Debug, Default)]
pub struct StunReconstructor {
    by_enemy: HashMap<u32, EnemyGauge>,
}

#[derive(Debug, Default)]
struct EnemyGauge {
    filled: f64,
    last_fill: Option<f32>,
}

impl StunReconstructor {
    /// A fill drop of at least this much between hits marks a break.
    const RESET_DROP: f32 = 0.3;

    /// At or above this, `stun_max` is the game's cannot-be-broken sentinel (1e9), not a capacity.
    const UNBREAKABLE_GAUGE: f32 = 1e8;

    pub fn counted_stun(&mut self, event: &DamageEvent) -> f64 {
        let raw = event.stun_value.unwrap_or(0.0) as f64;

        let max = match event.stun_max {
            Some(max) if max >= Self::UNBREAKABLE_GAUGE => return 0.0,
            Some(max) if max > 0.0 => max as f64,
            _ => return raw,
        };

        let gauge = self.by_enemy.entry(event.target.parent_index).or_default();

        if let (Some(fill), Some(last)) = (event.stun_fill, gauge.last_fill) {
            if last - fill >= Self::RESET_DROP {
                let counted = raw.min((max - gauge.filled).max(0.0));
                gauge.filled = (fill as f64 * max).max(0.0);
                gauge.last_fill = Some(fill);
                return counted;
            }
        }
        if let Some(fill) = event.stun_fill {
            gauge.last_fill = Some(fill);
        }

        let counted = raw.min((max - gauge.filled).max(0.0));
        gauge.filled += counted;
        counted
    }
}
