use protocol::ActionType;
use serde::{Deserialize, Serialize};

use crate::parser::constants::CharacterType;

use super::aura::AuraSource;
use super::AdjustedDamageInstance;

/// Derived stat breakdown of a particular skill
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillState {
    /// Type of action ID that this skill is
    pub action_type: ActionType,
    /// Child character this skill belongs to (pet, Id's dragonform, etc.)
    pub child_character_type: CharacterType,
    /// Conflux aura that produced this hit, for the several that share action id `99999`
    #[serde(default)]
    pub aura_source: AuraSource,
    /// Set when this row is the ether cannon rather than the character's
    /// own skill at action id `5000`/`5010`. See `ether_gun.rs`.
    #[serde(default)]
    pub ether_gun: bool,
    /// Number of hits this skill has done
    pub hits: u32,
    /// Minimum damage done by this skill
    pub min_damage: Option<u64>,
    /// Maximum damage done by this skill
    pub max_damage: Option<u64>,
    /// Total damage done by this skill
    pub total_damage: u64,
    /// Minimum non-zero stun value done by this skill
    pub min_stun_value: Option<f64>,
    /// Maximum non-zero stun value done by this skill
    pub max_stun_value: Option<f64>,
    /// Total stun value done by this skill
    pub total_stun_value: f64,
    /// Number of hits that dealt non-zero stun (denominator for stun-per-hit)
    pub stun_hits: u32,
}

/// Zero-damage hits (summon pulses, counter shots, pull ticks) still count for stun, but not for damage.
fn deals_damage(damage: i32) -> bool {
    damage > 0
}

impl SkillState {
    pub fn new(
        action_type: ActionType,
        child_character_type: CharacterType,
        aura_source: AuraSource,
        ether_gun: bool,
    ) -> Self {
        Self {
            action_type,
            child_character_type,
            aura_source,
            ether_gun,
            hits: 0,
            min_damage: None,
            max_damage: None,
            total_damage: 0,
            min_stun_value: None,
            max_stun_value: None,
            total_stun_value: 0.0,
            stun_hits: 0,
        }
    }

    pub fn update_from_damage_event(&mut self, damage_instance: &AdjustedDamageInstance) {
        self.total_stun_value += damage_instance.stun_damage;

        let stun_damage = damage_instance.stun_damage;
        if stun_damage > 0.0 {
            self.stun_hits += 1;

            if let Some(min_stun_value) = self.min_stun_value {
                self.min_stun_value = Some(min_stun_value.min(stun_damage));
            } else {
                self.min_stun_value = Some(stun_damage);
            }

            if let Some(max_stun_value) = self.max_stun_value {
                self.max_stun_value = Some(max_stun_value.max(stun_damage));
            } else {
                self.max_stun_value = Some(stun_damage);
            }
        }

        let damage = damage_instance.event.damage;
        if !deals_damage(damage) {
            return;
        }
        let damage = damage as u64;

        self.hits += 1;
        self.total_damage += damage;

        if let Some(min_damage) = self.min_damage {
            self.min_damage = Some(min_damage.min(damage));
        } else {
            self.min_damage = Some(damage);
        }

        if let Some(max_damage) = self.max_damage {
            self.max_damage = Some(max_damage.max(damage));
        } else {
            self.max_damage = Some(damage);
        }
    }
}

#[cfg(test)]
mod tests {
    use protocol::{Actor, DamageEvent};

    use super::*;

    #[test]
    fn updating_from_damage_event() {
        let mut skill_state = SkillState::new(
            ActionType::Normal(1),
            CharacterType::Pl0000,
            AuraSource::None,
            false,
        );

        let damage_event = DamageEvent {
            source: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            action_id: ActionType::Normal(1),
            damage: 100,
            flags: 0,
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
        };

        let damage_event_two = DamageEvent {
            source: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            action_id: ActionType::Normal(1),
            damage: 1999,
            flags: 0,
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
        };

        skill_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &damage_event,
            None,
        ));
        skill_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &damage_event_two,
            None,
        ));

        assert_eq!(skill_state.hits, 2);
        assert_eq!(skill_state.min_damage, Some(100));
        assert_eq!(skill_state.max_damage, Some(1999));
        assert_eq!(skill_state.total_damage, 2099);
    }

    #[test]
    fn zero_stun_hits_are_excluded_from_stun_hit_count() {
        let mut skill_state = SkillState::new(
            ActionType::Normal(1),
            CharacterType::Pl0000,
            AuraSource::None,
            false,
        );

        let base_event = DamageEvent {
            source: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            action_id: ActionType::Normal(1),
            damage: 100,
            flags: 0,
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
        };

        let staggering_hit = DamageEvent {
            stun_value: Some(5.0),
            ..base_event.clone()
        };
        let non_staggering_hit = DamageEvent {
            stun_value: Some(0.0),
            ..base_event.clone()
        };
        let no_stun_field_hit = DamageEvent {
            stun_value: None,
            ..base_event
        };

        skill_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &staggering_hit,
            None,
        ));
        skill_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &non_staggering_hit,
            None,
        ));
        skill_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &no_stun_field_hit,
            None,
        ));
        skill_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &staggering_hit,
            None,
        ));

        assert_eq!(skill_state.hits, 4);
        assert_eq!(skill_state.stun_hits, 2);
        assert_eq!(skill_state.total_stun_value, 10.0);
        assert_eq!(skill_state.min_stun_value, Some(5.0));
        assert_eq!(skill_state.max_stun_value, Some(5.0));
    }

    #[test]
    fn min_and_max_stun_value_track_both_bounds_across_hits() {
        let mut skill_state = SkillState::new(
            ActionType::Normal(1),
            CharacterType::Pl0000,
            AuraSource::None,
            false,
        );

        let base_event = DamageEvent {
            source: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            action_id: ActionType::Normal(1),
            damage: 100,
            flags: 0,
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
        };

        let small_stun_hit = DamageEvent {
            stun_value: Some(2.0),
            ..base_event.clone()
        };
        let large_stun_hit = DamageEvent {
            stun_value: Some(7.0),
            ..base_event.clone()
        };
        let non_staggering_hit = DamageEvent {
            stun_value: Some(0.0),
            ..base_event
        };

        skill_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &small_stun_hit,
            None,
        ));
        skill_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &non_staggering_hit,
            None,
        ));
        skill_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &large_stun_hit,
            None,
        ));

        assert_eq!(skill_state.stun_hits, 2);
        assert_eq!(skill_state.min_stun_value, Some(2.0));
        assert_eq!(skill_state.max_stun_value, Some(7.0));
    }

    #[test]
    fn zero_damage_hits_are_excluded_from_damage_stats_but_still_count_stun() {
        let mut skill_state = SkillState::new(
            ActionType::Normal(1),
            CharacterType::Pl0000,
            AuraSource::None,
            false,
        );

        let base_event = DamageEvent {
            source: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            action_id: ActionType::Normal(1),
            damage: 100,
            flags: 0,
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
        };

        let hit_a = DamageEvent {
            damage: 50,
            ..base_event.clone()
        };
        let hit_b = DamageEvent {
            damage: 200,
            ..base_event.clone()
        };
        let zero_damage_stun_tick = DamageEvent {
            damage: 0,
            stun_value: Some(3.0),
            ..base_event
        };

        skill_state
            .update_from_damage_event(&AdjustedDamageInstance::from_damage_event(&hit_a, None));
        skill_state
            .update_from_damage_event(&AdjustedDamageInstance::from_damage_event(&hit_b, None));
        skill_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &zero_damage_stun_tick,
            None,
        ));

        assert_eq!(skill_state.hits, 2);
        assert_eq!(skill_state.min_damage, Some(50));
        assert_eq!(skill_state.max_damage, Some(200));
        assert_eq!(skill_state.total_damage, 250);
        assert_eq!(skill_state.total_stun_value, 3.0);
        assert_eq!(skill_state.stun_hits, 1);
        assert_eq!(skill_state.min_stun_value, Some(3.0));
        assert_eq!(skill_state.max_stun_value, Some(3.0));
    }

    #[test]
    fn a_skill_that_only_ever_deals_zero_damage_reports_no_hits() {
        let mut skill_state = SkillState::new(
            ActionType::Normal(1),
            CharacterType::Pl0000,
            AuraSource::None,
            false,
        );

        let zero_damage_stun_tick = DamageEvent {
            source: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            action_id: ActionType::Normal(1),
            damage: 0,
            flags: 0,
            attack_rate: None,
            stun_value: Some(5.0),
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
        };

        skill_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &zero_damage_stun_tick,
            None,
        ));

        assert_eq!(skill_state.hits, 0);
        assert_eq!(skill_state.min_damage, None);
        assert_eq!(skill_state.max_damage, None);
        assert_eq!(skill_state.total_damage, 0);
        assert_eq!(skill_state.total_stun_value, 5.0);
        assert_eq!(skill_state.min_stun_value, Some(5.0));
        assert_eq!(skill_state.max_stun_value, Some(5.0));
    }
}
