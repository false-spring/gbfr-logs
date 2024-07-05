use protocol::{ActionType, DamageEvent};
use serde::{Deserialize, Serialize};

use crate::parser::constants::CharacterType;

use super::PlayerData;

/// Derived stat breakdown of a particular skill
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillState {
    /// Type of action ID that this skill is
    pub action_type: ActionType,
    /// Child character this skill belongs to (pet, Id's dragonform, etc.)
    pub child_character_type: CharacterType,
    /// Number of hits this skill has done
    pub hits: u32,
    /// Minimum damage done by this skill
    pub min_damage: Option<u64>,
    /// Maximum damage done by this skill
    pub max_damage: Option<u64>,
    /// Total damage done by this skill
    pub total_damage: u64,
    /// Maximum stun value done by this skill
    pub max_stun_value: f64,
    /// Total stun value done by this skill
    pub total_stun_value: f64,
}

impl SkillState {
    pub fn new(action_type: ActionType, child_character_type: CharacterType) -> Self {
        Self {
            action_type,
            child_character_type,
            hits: 0,
            min_damage: None,
            max_damage: None,
            total_damage: 0,
            max_stun_value: 0.0,
            total_stun_value: 0.0,
        }
    }

    pub fn update_from_damage_event(
        &mut self,
        event: &DamageEvent,
        player_data: Option<&PlayerData>,
    ) {
        self.hits += 1;
        self.total_damage += event.damage as u64;

        let stun_modifier = player_data
            .and_then(|data| Some(data.stun_modifier()))
            .unwrap_or(10.0);

        let stun_value = event.stun_value.unwrap_or(0.0) as f64;

        self.max_stun_value = self.max_stun_value.max(stun_value * stun_modifier);
        self.total_stun_value += stun_value * stun_modifier;

        if let Some(min_damage) = self.min_damage {
            self.min_damage = Some(min_damage.min(event.damage as u64));
        } else {
            self.min_damage = Some(event.damage as u64);
        }

        if let Some(max_damage) = self.max_damage {
            self.max_damage = Some(max_damage.max(event.damage as u64));
        } else {
            self.max_damage = Some(event.damage as u64);
        }
    }
}

#[cfg(test)]
mod tests {
    use protocol::Actor;

    use super::*;

    #[test]
    fn updating_from_damage_event() {
        let mut skill_state = SkillState::new(ActionType::Normal(1), CharacterType::Pl0000);

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
        };

        skill_state.update_from_damage_event(&damage_event, None);
        skill_state.update_from_damage_event(&damage_event_two, None);

        assert_eq!(skill_state.hits, 2);
        assert_eq!(skill_state.min_damage, Some(100));
        assert_eq!(skill_state.max_damage, Some(1999));
        assert_eq!(skill_state.total_damage, 2099);
    }
}
