use protocol::{ActionType, DamageEvent};
use serde::{Deserialize, Serialize};

use crate::parser::constants::{CharacterType, FerrySkillId};

use super::skill_state::SkillState;

/// Derived stat breakdown for a player
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub index: u32,
    pub character_type: CharacterType,
    pub total_damage: u64,
    pub last_known_pet_skill: Option<ActionType>, // used for Ferry's skills that don't keep track of where they came from
    pub dps: f64,
    pub skill_breakdown: Vec<SkillState>,
    pub sba: f64,
}

impl PlayerState {
    pub fn set_sba(&mut self, sba: f64) {
        self.sba = sba;
    }

    pub fn update_dps(&mut self, now: i64, start_time: i64) {
        self.dps = self.total_damage as f64 / ((now - start_time) as f64 / 1000.0);
    }

    // @todo(false): maybe Ferry specific stuff can be removed/abstracted if some extra flags are found or the attribution is fixed
    pub fn get_action_from_ferry_damage_event(&mut self, event: &DamageEvent) -> ActionType {
        // Ferry needs special handling because the action_id that comes back for pet skills is usually wrong
        // e.g. if you strafe then dodge the action_id for further hits comes back as "dodge"
        let is_ferry_pet =
            CharacterType::Pl0700Ghost == CharacterType::from_hash(event.source.actor_type);
        let is_ferry_pet_skill = is_ferry_pet && (event.flags & (1 << 2) != 0); // pet skills for ferry always have this flag set
        let is_ferry_pet_normal =
            is_ferry_pet && !is_ferry_pet_skill && event.action_id != ActionType::LinkAttack;

        // Umlauf excluded since that uses a separate actor which works correctly
        if is_ferry_pet_skill
            && vec![
                FerrySkillId::BlausGespenst,
                FerrySkillId::Pendel,
                FerrySkillId::Strafe,
            ]
            .into_iter()
            .any(|skill_id| ActionType::Normal(skill_id as u32) == event.action_id)
        {
            self.last_known_pet_skill = Some(event.action_id);
        }
        const PET_NORMAL: ActionType = ActionType::Normal(FerrySkillId::PetNormal as u32);
        let action = if is_ferry_pet_normal {
            // Note technically the pet portion of Onslaught will count as a Pet normal, but I think that's fine since
            // it does exactly as much as a pet normal. Could consider adding Onslaught (pet) as a separate category
            PET_NORMAL
        } else if is_ferry_pet_skill {
            match self.last_known_pet_skill {
                None => PET_NORMAL, // May be good to instead have a separate "pet skill" backup for this case
                Some(skill_id) => skill_id,
            }
        } else {
            event.action_id
        };
        return action;
    }

    pub fn update_from_damage_event(&mut self, event: &DamageEvent) {
        self.total_damage += event.damage as u64;

        let parent_character_type = CharacterType::from_hash(event.source.parent_actor_type);

        // @TODO(false): Collapse all skill IDs from Seofon's avatar into his own.
        let child_character_type = if parent_character_type == CharacterType::Pl2200 {
            parent_character_type
        } else {
            CharacterType::from_hash(event.source.actor_type)
        };

        // for ferry defer to special function to handle the weird way her pets work
        let action = if parent_character_type == CharacterType::Pl0700 {
            self.get_action_from_ferry_damage_event(event)
        } else {
            event.action_id
        };

        // If the skill is already being tracked, update it.
        for skill in self.skill_breakdown.iter_mut() {
            // Aggregate all supplementary damage events into the same skill instance.
            if matches!(
                skill.action_type,
                protocol::ActionType::SupplementaryDamage(_)
            ) && matches!(action, protocol::ActionType::SupplementaryDamage(_))
            {
                skill.update_from_damage_event(event);
                return;
            }

            // If the skill is already being tracked, update it.
            if skill.action_type == action && skill.child_character_type == child_character_type {
                skill.update_from_damage_event(event);
                return;
            }
        }

        // Otherwise, create a new skill and track it.
        let mut skill = SkillState::new(action, child_character_type);

        skill.update_from_damage_event(event);
        self.skill_breakdown.push(skill);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_dps() {
        let mut player_state = PlayerState {
            index: 0,
            character_type: CharacterType::Pl0000,
            total_damage: 100,
            last_known_pet_skill: None,
            dps: 0.0,
            skill_breakdown: vec![],
            sba: 0.0,
        };

        player_state.update_dps(1000, 0);

        assert_eq!(player_state.dps, 100.0);
    }

    #[test]
    fn updates_from_damage_event() {
        let mut player_state = PlayerState {
            index: 0,
            character_type: CharacterType::Pl0000,
            total_damage: 0,
            last_known_pet_skill: None,
            dps: 0.0,
            skill_breakdown: vec![],
            sba: 0.0,
        };

        let damage_event = DamageEvent {
            source: protocol::Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: protocol::Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            action_id: ActionType::Normal(1),
            damage: 100,
            flags: 0,
        };

        player_state.update_from_damage_event(&damage_event);

        assert_eq!(player_state.total_damage, 100);
        assert_eq!(player_state.skill_breakdown.len(), 1);
        assert_eq!(player_state.skill_breakdown[0].total_damage, 100);
    }

    #[test]
    fn same_skill_updates_from_multiple_damage_events() {
        let mut player_state = PlayerState {
            index: 0,
            character_type: CharacterType::Pl0000,
            total_damage: 0,
            last_known_pet_skill: None,
            dps: 0.0,
            skill_breakdown: vec![],
            sba: 0.0,
        };

        let damage_event = DamageEvent {
            source: protocol::Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: protocol::Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            action_id: ActionType::Normal(1),
            damage: 100,
            flags: 0,
        };

        player_state.update_from_damage_event(&damage_event);
        player_state.update_from_damage_event(&damage_event);
        player_state.update_from_damage_event(&damage_event);

        assert_eq!(player_state.total_damage, 300);
        assert_eq!(player_state.skill_breakdown.len(), 1);
        assert_eq!(player_state.skill_breakdown[0].total_damage, 300);
    }

    #[test]
    fn new_skills_are_tracked_separately() {
        let mut player_state = PlayerState {
            index: 0,
            character_type: CharacterType::Pl0000,
            total_damage: 0,
            last_known_pet_skill: None,
            dps: 0.0,
            skill_breakdown: vec![],
            sba: 0.0,
        };

        let skill_one = DamageEvent {
            source: protocol::Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: protocol::Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            action_id: ActionType::Normal(1),
            damage: 100,
            flags: 0,
        };

        let skill_two = DamageEvent {
            source: protocol::Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: protocol::Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            action_id: ActionType::Normal(2),
            damage: 100,
            flags: 0,
        };

        player_state.update_from_damage_event(&skill_one);
        player_state.update_from_damage_event(&skill_two);
        player_state.update_from_damage_event(&skill_two);

        assert_eq!(player_state.total_damage, 300);
        assert_eq!(player_state.skill_breakdown.len(), 2);
        assert_eq!(player_state.skill_breakdown[0].total_damage, 100);
        assert_eq!(player_state.skill_breakdown[1].total_damage, 200);
    }

    fn skills_from_children_are_tracked_separately() {
        let mut player_state = PlayerState {
            index: 0,
            character_type: CharacterType::Pl0000,
            total_damage: 0,
            last_known_pet_skill: None,
            dps: 0.0,
            skill_breakdown: vec![],
            sba: 0.0,
        };

        let parent_skill = DamageEvent {
            source: protocol::Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: protocol::Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            action_id: ActionType::Normal(1),
            damage: 100,
            flags: 0,
        };

        let child_skill = DamageEvent {
            source: protocol::Actor {
                index: 1,
                actor_type: 1,
                parent_actor_type: 0,
                parent_index: 0,
            },
            target: protocol::Actor {
                index: 0,
                actor_type: 0,
                parent_actor_type: 0,
                parent_index: 0,
            },
            action_id: ActionType::Normal(1),
            damage: 100,
            flags: 0,
        };

        player_state.update_from_damage_event(&parent_skill);
        player_state.update_from_damage_event(&child_skill);
        player_state.update_from_damage_event(&child_skill);

        assert_eq!(player_state.total_damage, 200);
        assert_eq!(player_state.skill_breakdown.len(), 2);
        assert_eq!(player_state.skill_breakdown[0].total_damage, 100);
        assert_eq!(player_state.skill_breakdown[1].total_damage, 200);
    }
}
