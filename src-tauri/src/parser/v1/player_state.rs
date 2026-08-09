use protocol::{ActionType, DamageEvent, HealEvent, SbaCause};
use serde::{Deserialize, Serialize};

use crate::parser::constants::{CharacterType, FerrySkillId};

use super::aura::{aura_source_of, CONFLUX_AURA_SENTINEL};
use super::ether_gun::is_ether_gun;
use super::{sba_state::SbaSourceState, skill_state::SkillState, AdjustedDamageInstance};

/// `HealInfo +0x0C` for the generic heal skill; every other producer passes `0xFFFFFFFF`.
const PLAYER_HEAL_HP_ACTION_CHANNEL: u32 = 0x0001_3880;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HealCategory {
    Skill,
    /// Drain and potion arrive identical at the hook, so they share one bucket.
    SelfRecovery,
    Regen,
    Revive,
    Other,
}

pub(super) fn classify_heal(event: &HealEvent) -> HealCategory {
    if event.heal_type == 4 {
        HealCategory::Revive
    } else if event.channel == PLAYER_HEAL_HP_ACTION_CHANNEL {
        HealCategory::Skill
    } else if event.via_apply && event.heal_type == 0 {
        HealCategory::Skill
    } else if !event.via_apply && event.heal_type == 2 {
        HealCategory::SelfRecovery
    } else if !event.via_apply && event.heal_type == 1 {
        HealCategory::Regen
    } else {
        HealCategory::Other
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct HealBreakdown {
    pub skill: u64,
    pub self_recovery: u64,
    pub regen: u64,
    pub revive: u64,
    pub other: u64,
}

impl HealBreakdown {
    pub(super) fn add(&mut self, category: HealCategory, amount: u64) {
        match category {
            HealCategory::Skill => self.skill += amount,
            HealCategory::SelfRecovery => self.self_recovery += amount,
            HealCategory::Regen => self.regen += amount,
            HealCategory::Revive => self.revive += amount,
            HealCategory::Other => self.other += amount,
        }
    }
}

pub(super) fn child_character_type_of(event: &DamageEvent) -> CharacterType {
    let parent_character_type = CharacterType::from_hash(event.source.parent_actor_type);

    // @TODO(false): Collapse all skill IDs from Seofon's avatar into his own.
    if parent_character_type == CharacterType::Pl2200 {
        parent_character_type
    } else {
        CharacterType::from_hash(event.source.actor_type)
    }
}

/// Derived stat breakdown for a player
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub index: u32,
    pub character_type: CharacterType,
    pub total_damage: u64,
    pub last_known_pet_skill: Option<ActionType>, // used for Ferry's skills that don't keep track of where they came from
    #[serde(skip)]
    pub last_non_sentinel_action: Option<ActionType>,
    pub dps: f64,
    pub skill_breakdown: Vec<SkillState>,
    pub sba: f64,
    pub total_stun_value: f64,
    pub stun_per_second: f64,
    pub total_damage_taken: u64,
    pub heal_done: u64,
    pub heal_received: u64,
    pub heal_provided_by_type: HealBreakdown,
    pub heal_received_by_type: HealBreakdown,
    pub sba_breakdown: Vec<SbaSourceState>,
    pub total_sba_added: f64,
}

impl PlayerState {
    pub fn set_sba(&mut self, sba: f64) {
        self.sba = sba;
    }

    pub fn add_sba_gain(
        &mut self,
        cause: SbaCause,
        child_character_type: Option<CharacterType>,
        sba_added: f64,
    ) {
        if sba_added <= 0.0 {
            return;
        }

        self.total_sba_added += sba_added;

        for source in self.sba_breakdown.iter_mut() {
            if source.cause == cause && source.child_character_type == child_character_type {
                source.ticks += 1;
                source.total_sba_added += sba_added;
                return;
            }
        }

        self.sba_breakdown.push(SbaSourceState {
            cause,
            child_character_type,
            ticks: 1,
            total_sba_added: sba_added,
        });
    }

    pub fn update_dps(&mut self, now: i64, start_time: i64) {
        let elapsed_secs = (now - start_time).max(1) as f64 / 1000.0;
        self.dps = self.total_damage as f64 / elapsed_secs;
        self.stun_per_second = self.total_stun_value / elapsed_secs;
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

        if is_ferry_pet_normal {
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
        }
    }

    pub fn update_from_damage_event(&mut self, damage_instance: &AdjustedDamageInstance) {
        self.total_damage += damage_instance.event.damage as u64;
        self.total_stun_value += damage_instance.stun_damage;

        let parent_character_type =
            CharacterType::from_hash(damage_instance.event.source.parent_actor_type);

        let child_character_type = child_character_type_of(damage_instance.event);

        // for ferry defer to special function to handle the weird way her pets work
        let action = if parent_character_type == CharacterType::Pl0700 {
            self.get_action_from_ferry_damage_event(damage_instance.event)
        } else {
            damage_instance.event.action_id
        };

        let aura_source = aura_source_of(
            action,
            damage_instance.event.flags,
            self.last_non_sentinel_action,
        );

        if action != ActionType::Normal(CONFLUX_AURA_SENTINEL) {
            self.last_non_sentinel_action = Some(action);
        }

        // Five characters own a skill at action id 5000/5010, which is also
        // what Endless Ragnarok's shared ether gun reports — so the id alone
        // names the gun after whichever skill the character happens to own.
        // Returns `false` for every other action on the first comparison.
        let ether_gun = is_ether_gun(action, damage_instance.event.flags);

        // If the skill is already being tracked, update it.
        for skill in self.skill_breakdown.iter_mut() {
            // Aggregate all supplementary damage events into the same skill instance.
            if matches!(
                skill.action_type,
                protocol::ActionType::SupplementaryDamage(_)
            ) && matches!(action, protocol::ActionType::SupplementaryDamage(_))
            {
                skill.update_from_damage_event(damage_instance);
                return;
            }

            // If the skill is already being tracked, update it.
            if skill.action_type == action
                && skill.child_character_type == child_character_type
                && skill.aura_source == aura_source
                && skill.ether_gun == ether_gun
            {
                skill.update_from_damage_event(damage_instance);
                return;
            }
        }

        // Otherwise, create a new skill and track it.
        let mut skill = SkillState::new(action, child_character_type, aura_source, ether_gun);

        skill.update_from_damage_event(damage_instance);
        self.skill_breakdown.push(skill);
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::v1::{aura::AuraSource, dto::PlayerStats, PlayerData};

    use super::*;

    #[test]
    fn calculates_dps() {
        let mut player_state = PlayerState {
            index: 0,
            character_type: CharacterType::Pl0000,
            total_damage: 100,
            last_known_pet_skill: None,
            last_non_sentinel_action: None,
            dps: 0.0,
            skill_breakdown: vec![],
            sba: 0.0,
            total_stun_value: 0.0,
            stun_per_second: 0.0,
            total_damage_taken: 0,
            heal_done: 0,
            heal_received: 0,
            heal_provided_by_type: HealBreakdown::default(),
            heal_received_by_type: HealBreakdown::default(),
            sba_breakdown: vec![],
            total_sba_added: 0.0,
        };

        player_state.update_dps(1000, 0);

        assert_eq!(player_state.dps, 100.0);
    }

    #[test]
    fn dps_does_not_overflow_on_the_encounter_opening_hit() {
        let mut player_state = PlayerState {
            index: 0,
            character_type: CharacterType::Pl0000,
            total_damage: 620652,
            last_known_pet_skill: None,
            last_non_sentinel_action: None,
            dps: 0.0,
            skill_breakdown: vec![],
            sba: 0.0,
            total_stun_value: 57.0,
            stun_per_second: 0.0,
            total_damage_taken: 0,
            heal_done: 0,
            heal_received: 0,
            heal_provided_by_type: HealBreakdown::default(),
            heal_received_by_type: HealBreakdown::default(),
            sba_breakdown: vec![],
            total_sba_added: 0.0,
        };

        player_state.update_dps(1000, 1000);

        assert!(player_state.dps.is_finite());
        assert!(player_state.stun_per_second.is_finite());
    }

    #[test]
    fn updates_from_damage_event() {
        let mut player_state = PlayerState {
            index: 0,
            character_type: CharacterType::Pl0000,
            total_damage: 0,
            last_known_pet_skill: None,
            last_non_sentinel_action: None,
            dps: 0.0,
            skill_breakdown: vec![],
            sba: 0.0,
            total_stun_value: 0.0,
            stun_per_second: 0.0,
            total_damage_taken: 0,
            heal_done: 0,
            heal_received: 0,
            heal_provided_by_type: HealBreakdown::default(),
            heal_received_by_type: HealBreakdown::default(),
            sba_breakdown: vec![],
            total_sba_added: 0.0,
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
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
            hit_calc: None,
        };

        player_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &damage_event,
            None,
        ));

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
            last_non_sentinel_action: None,
            dps: 0.0,
            skill_breakdown: vec![],
            sba: 0.0,
            total_stun_value: 0.0,
            stun_per_second: 0.0,
            total_damage_taken: 0,
            heal_done: 0,
            heal_received: 0,
            heal_provided_by_type: HealBreakdown::default(),
            heal_received_by_type: HealBreakdown::default(),
            sba_breakdown: vec![],
            total_sba_added: 0.0,
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
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
            hit_calc: None,
        };

        player_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &damage_event,
            None,
        ));
        player_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &damage_event,
            None,
        ));
        player_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &damage_event,
            None,
        ));

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
            last_non_sentinel_action: None,
            dps: 0.0,
            skill_breakdown: vec![],
            sba: 0.0,
            stun_per_second: 0.0,
            total_damage_taken: 0,
            heal_done: 0,
            heal_received: 0,
            heal_provided_by_type: HealBreakdown::default(),
            heal_received_by_type: HealBreakdown::default(),
            sba_breakdown: vec![],
            total_sba_added: 0.0,
            total_stun_value: 0.0,
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
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
            hit_calc: None,
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
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
            hit_calc: None,
        };

        player_state
            .update_from_damage_event(&AdjustedDamageInstance::from_damage_event(&skill_one, None));
        player_state
            .update_from_damage_event(&AdjustedDamageInstance::from_damage_event(&skill_two, None));
        player_state
            .update_from_damage_event(&AdjustedDamageInstance::from_damage_event(&skill_two, None));

        assert_eq!(player_state.total_damage, 300);
        assert_eq!(player_state.skill_breakdown.len(), 2);
        assert_eq!(player_state.skill_breakdown[0].total_damage, 100);
        assert_eq!(player_state.skill_breakdown[1].total_damage, 200);
    }

    #[test]
    fn skills_from_children_are_tracked_separately() {
        let mut player_state = PlayerState {
            index: 0,
            character_type: CharacterType::Pl0000,
            total_damage: 0,
            last_known_pet_skill: None,
            last_non_sentinel_action: None,
            dps: 0.0,
            skill_breakdown: vec![],
            sba: 0.0,
            stun_per_second: 0.0,
            total_damage_taken: 0,
            heal_done: 0,
            heal_received: 0,
            heal_provided_by_type: HealBreakdown::default(),
            heal_received_by_type: HealBreakdown::default(),
            sba_breakdown: vec![],
            total_sba_added: 0.0,
            total_stun_value: 0.0,
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
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
            hit_calc: None,
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
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
            hit_calc: None,
        };

        player_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &parent_skill,
            None,
        ));
        player_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &child_skill,
            None,
        ));
        player_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &child_skill,
            None,
        ));

        assert_eq!(player_state.total_damage, 300);
        assert_eq!(player_state.skill_breakdown.len(), 2);
        assert_eq!(player_state.skill_breakdown[0].total_damage, 100);
        assert_eq!(player_state.skill_breakdown[1].total_damage, 200);
    }

    #[test]
    fn stun_is_tracked_with_player_stats() {
        let mut player_state = PlayerState {
            index: 0,
            character_type: CharacterType::Pl0000,
            total_damage: 0,
            last_known_pet_skill: None,
            last_non_sentinel_action: None,
            dps: 0.0,
            skill_breakdown: vec![],
            sba: 0.0,
            total_stun_value: 0.0,
            stun_per_second: 0.0,
            total_damage_taken: 0,
            heal_done: 0,
            heal_received: 0,
            heal_provided_by_type: HealBreakdown::default(),
            heal_received_by_type: HealBreakdown::default(),
            sba_breakdown: vec![],
            total_sba_added: 0.0,
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
            attack_rate: None,
            stun_value: Some(5.0),
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
            hit_calc: None,
        };

        let player_data = PlayerData {
            actor_index: 0,
            character_type: CharacterType::Pl0000,
            display_name: "Test".to_string(),
            character_name: "Test".to_string(),
            sigils: Vec::new(),
            is_online: false,
            weapon_info: None,
            overmastery_info: None,
            summon_info: None,
            skill_loadout: Vec::new(),
            over_mastery: Vec::new(),
            master_trait_flags: Vec::new(),
            effective_traits: Vec::new(),
            player_stats: Some(PlayerStats {
                level: 100,
                total_hp: 10000,
                total_attack: 1000,
                stun_power: 130.0,
                critical_rate: 100.0,
                total_power: 1000,
                dmg_cap_channels: [0.0; 3],
            }),
            network_user_id: None,
            network_user_name: None,
            master_level: None,
        };

        player_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &damage_event,
            Some(&player_data),
        ));

        assert_eq!(player_state.total_stun_value, 5.0);
    }

    #[test]
    fn stun_value_without_player_stats() {
        let mut player_state = PlayerState {
            index: 0,
            character_type: CharacterType::Pl0000,
            total_damage: 0,
            last_known_pet_skill: None,
            last_non_sentinel_action: None,
            dps: 0.0,
            skill_breakdown: vec![],
            sba: 0.0,
            total_stun_value: 0.0,
            stun_per_second: 0.0,
            total_damage_taken: 0,
            heal_done: 0,
            heal_received: 0,
            heal_provided_by_type: HealBreakdown::default(),
            heal_received_by_type: HealBreakdown::default(),
            sba_breakdown: vec![],
            total_sba_added: 0.0,
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
            attack_rate: None,
            stun_value: Some(5.0),
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
            hit_calc: None,
        };

        player_state.update_from_damage_event(&AdjustedDamageInstance::from_damage_event(
            &damage_event,
            None,
        ));

        assert_eq!(player_state.total_stun_value, 5.0);
    }

    #[test]
    fn conflux_auras_sharing_the_sentinel_id_do_not_merge() {
        let mut player_state = PlayerState {
            index: 0,
            character_type: CharacterType::Pl0000,
            total_damage: 0,
            last_known_pet_skill: None,
            last_non_sentinel_action: None,
            dps: 0.0,
            skill_breakdown: vec![],
            sba: 0.0,
            stun_per_second: 0.0,
            total_damage_taken: 0,
            heal_done: 0,
            heal_received: 0,
            heal_provided_by_type: HealBreakdown::default(),
            heal_received_by_type: HealBreakdown::default(),
            sba_breakdown: vec![],
            total_sba_added: 0.0,
            total_stun_value: 0.0,
        };

        let toxic_blast = aura_hit(0x1080100002020000, 100);
        let luster = aura_hit(0x1000100002040000, 40);

        for event in [&toxic_blast, &toxic_blast, &luster] {
            player_state
                .update_from_damage_event(&AdjustedDamageInstance::from_damage_event(event, None));
        }

        assert_eq!(player_state.skill_breakdown.len(), 2);
        assert_eq!(player_state.total_damage, 240);

        let toxic = player_state
            .skill_breakdown
            .iter()
            .find(|s| s.aura_source == AuraSource::ToxicBlast)
            .expect("Toxic Blast row");
        assert_eq!(toxic.hits, 2);
        assert_eq!(toxic.total_damage, 200);

        let luster_row = player_state
            .skill_breakdown
            .iter()
            .find(|s| s.aura_source == AuraSource::LusterOfDarkness)
            .expect("Luster of Darkness row");
        assert_eq!(luster_row.hits, 1);
        assert_eq!(luster_row.total_damage, 40);
    }

    #[test]
    fn the_ether_gun_does_not_merge_with_the_skill_sharing_its_id() {
        let mut player_state = blank_player_state();

        let gun = shared_id_hit(5000, 0x0000_1000_0100_0000, 8000);
        let hearts_on_fire = shared_id_hit(5000, 0x0000_0000_0002_0800, 50000);

        for event in [&gun, &gun, &hearts_on_fire] {
            player_state
                .update_from_damage_event(&AdjustedDamageInstance::from_damage_event(event, None));
        }

        assert_eq!(player_state.skill_breakdown.len(), 2);
        assert_eq!(player_state.total_damage, 66000);

        let gun_row = player_state
            .skill_breakdown
            .iter()
            .find(|s| s.ether_gun)
            .expect("ether gun row");
        assert_eq!(gun_row.hits, 2);
        assert_eq!(gun_row.total_damage, 16000);

        let skill_row = player_state
            .skill_breakdown
            .iter()
            .find(|s| !s.ether_gun)
            .expect("Hearts on Fire row");
        assert_eq!(skill_row.hits, 1);
        assert_eq!(skill_row.total_damage, 50000);
    }

    #[test]
    fn the_guns_two_shots_remain_distinct_rows() {
        let mut player_state = blank_player_state();

        let ether_round = shared_id_hit(5000, 0x0000_1000_0100_0000, 8000);
        let charged_shot = shared_id_hit(5010, 0x0000_1000_0100_0000, 20000);

        for event in [&ether_round, &charged_shot] {
            player_state
                .update_from_damage_event(&AdjustedDamageInstance::from_damage_event(event, None));
        }

        assert_eq!(player_state.skill_breakdown.len(), 2);
        assert!(player_state.skill_breakdown.iter().all(|s| s.ether_gun));
    }

    /// The discriminator must not change how anything else is grouped: repeats
    /// of one ordinary move stay a single row whatever their flags say.
    #[test]
    fn ordinary_moves_are_unaffected_by_flags() {
        let mut player_state = PlayerState {
            index: 0,
            character_type: CharacterType::Pl0000,
            total_damage: 0,
            last_known_pet_skill: None,
            last_non_sentinel_action: None,
            dps: 0.0,
            skill_breakdown: vec![],
            sba: 0.0,
            stun_per_second: 0.0,
            total_damage_taken: 0,
            heal_done: 0,
            heal_received: 0,
            heal_provided_by_type: HealBreakdown::default(),
            heal_received_by_type: HealBreakdown::default(),
            sba_breakdown: vec![],
            total_sba_added: 0.0,
            total_stun_value: 0.0,
        };

        for flags in [0x2020000, 0x1080100002020000, 0x1000100002040000] {
            let event = ordinary_hit(flags);
            player_state
                .update_from_damage_event(&AdjustedDamageInstance::from_damage_event(&event, None));
        }

        assert_eq!(player_state.skill_breakdown.len(), 1);
        assert_eq!(player_state.skill_breakdown[0].hits, 3);
    }

    #[test]
    fn every_tail_of_one_ice_and_fire_proc_is_attributed_to_it() {
        let mut player_state = blank_player_state();

        let ice_and_fire = DamageEvent {
            action_id: ActionType::Normal(100008),
            damage: 1000,
            ..blank_event()
        };
        let tail = aura_hit(0x1000100002040000, 350);

        for event in [&ice_and_fire, &tail, &tail] {
            player_state
                .update_from_damage_event(&AdjustedDamageInstance::from_damage_event(event, None));
        }

        assert_eq!(player_state.skill_breakdown.len(), 2);
        assert!(player_state
            .skill_breakdown
            .iter()
            .all(|s| s.aura_source != AuraSource::LusterOfDarkness));

        let follow_up = player_state
            .skill_breakdown
            .iter()
            .find(|s| s.aura_source == AuraSource::IceAndFireFollowUp)
            .expect("Ice and Fire follow-up row");
        assert_eq!(follow_up.hits, 2);
        assert_eq!(follow_up.total_damage, 700);
    }

    #[test]
    fn a_later_hit_releases_the_ice_and_fire_gate() {
        let mut player_state = blank_player_state();

        let ice_and_fire = DamageEvent {
            action_id: ActionType::Normal(100008),
            damage: 1000,
            ..blank_event()
        };
        let ordinary = ordinary_hit(0x2020000);
        let luster = aura_hit(0x1000100002040000, 350);

        for event in [&ice_and_fire, &luster, &ordinary, &luster] {
            player_state
                .update_from_damage_event(&AdjustedDamageInstance::from_damage_event(event, None));
        }

        let follow_up = player_state
            .skill_breakdown
            .iter()
            .find(|s| s.aura_source == AuraSource::IceAndFireFollowUp)
            .expect("Ice and Fire follow-up row");
        assert_eq!(follow_up.hits, 1);

        let luster_row = player_state
            .skill_breakdown
            .iter()
            .find(|s| s.aura_source == AuraSource::LusterOfDarkness)
            .expect("Luster of Darkness row");
        assert_eq!(luster_row.hits, 1);
    }

    fn blank_player_state() -> PlayerState {
        PlayerState {
            index: 0,
            character_type: CharacterType::Pl0000,
            total_damage: 0,
            last_known_pet_skill: None,
            last_non_sentinel_action: None,
            dps: 0.0,
            skill_breakdown: vec![],
            sba: 0.0,
            stun_per_second: 0.0,
            total_damage_taken: 0,
            heal_done: 0,
            heal_received: 0,
            heal_provided_by_type: HealBreakdown::default(),
            heal_received_by_type: HealBreakdown::default(),
            sba_breakdown: vec![],
            total_sba_added: 0.0,
            total_stun_value: 0.0,
        }
    }

    fn shared_id_hit(action_id: u32, flags: u64, damage: i32) -> DamageEvent {
        DamageEvent {
            action_id: ActionType::Normal(action_id),
            damage,
            flags,
            ..blank_event()
        }
    }

    /// A bare damage event carrying the Conflux sentinel id.
    fn aura_hit(flags: u64, damage: i32) -> DamageEvent {
        DamageEvent {
            action_id: ActionType::Normal(99999),
            damage,
            flags,
            ..blank_event()
        }
    }

    fn ordinary_hit(flags: u64) -> DamageEvent {
        DamageEvent {
            action_id: ActionType::Normal(200),
            damage: 50,
            flags,
            ..blank_event()
        }
    }

    fn blank_event() -> DamageEvent {
        DamageEvent {
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
            action_id: ActionType::Normal(0),
            damage: 0,
            flags: 0,
            attack_rate: None,
            stun_value: None,
            damage_cap: None,
            stun_fill: None,
            target_base_type: None,
            stun_max: None,
            hit_calc: None,
        }
    }
}
