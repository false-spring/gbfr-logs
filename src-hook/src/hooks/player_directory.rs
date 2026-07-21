//! Player identity stores and the encounter boundaries that clear them.
//!
//! Lock order: a thread holding IDENTITY_STORE may then take NETWORK_USER_STORE
//! or LOCAL_NETWORK_USER (`build_roster` in player.rs). The reverse never
//! happens and nothing else nests, so there is no cycle.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::gbfr_hash::gbfr_hash;
use super::network_user::NetworkUser;
use super::player::PlayerIdentity;

static IDENTITY_STORE: OnceLock<Mutex<HashMap<(u8, usize), PlayerIdentity>>> = OnceLock::new();

static ANNOUNCED_IDENTITIES: OnceLock<Mutex<HashMap<u32, u8>>> = OnceLock::new();

static LAST_ROSTER_SIG: OnceLock<Mutex<Vec<(u8, u32, Vec<u8>)>>> = OnceLock::new();

pub(crate) fn identity_store() -> &'static Mutex<HashMap<(u8, usize), PlayerIdentity>> {
    IDENTITY_STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn announced_identities() -> &'static Mutex<HashMap<u32, u8>> {
    ANNOUNCED_IDENTITIES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn last_roster_sig() -> &'static Mutex<Vec<(u8, u32, Vec<u8>)>> {
    LAST_ROSTER_SIG.get_or_init(|| Mutex::new(Vec::new()))
}

pub(crate) fn reset_announced() {
    if let Ok(mut announced) = announced_identities().lock() {
        announced.clear();
    }
}

fn clear_identity_and_roster() {
    if let Ok(mut store) = identity_store().lock() {
        store.clear();
    }
    if let Ok(mut sig) = last_roster_sig().lock() {
        sig.clear();
    }
    reset_announced();
}

// Never cleared: populated only while the lobby roster is built, before the quest loads.
static NETWORK_USER_STORE: OnceLock<Mutex<HashMap<u8, NetworkUser>>> = OnceLock::new();

static LOCAL_NETWORK_USER: OnceLock<Mutex<Option<NetworkUser>>> = OnceLock::new();

fn network_user_store() -> &'static Mutex<HashMap<u8, NetworkUser>> {
    NETWORK_USER_STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn local_network_user_cell() -> &'static Mutex<Option<NetworkUser>> {
    LOCAL_NETWORK_USER.get_or_init(|| Mutex::new(None))
}

pub(crate) fn network_user_for_enw(enw: u8) -> Option<NetworkUser> {
    network_user_store().lock().ok()?.get(&enw).cloned()
}

pub(crate) fn local_network_user() -> Option<NetworkUser> {
    local_network_user_cell().lock().ok()?.clone()
}

pub(crate) fn set_network_user(enw: u8, user: NetworkUser) {
    if let Ok(mut store) = network_user_store().lock() {
        store.insert(enw, user);
    }
}

pub(crate) fn set_local_network_user(user: NetworkUser) {
    if let Ok(mut local) = local_network_user_cell().lock() {
        *local = Some(user);
    }
}

static KEY_TO_CHARACTER: OnceLock<HashMap<u32, u32>> = OnceLock::new();

fn key_to_character() -> &'static HashMap<u32, u32> {
    KEY_TO_CHARACTER.get_or_init(|| {
        let mut map = HashMap::new();
        for n in (0..=9900).step_by(100) {
            let suffix = format!("{:04}", n);
            let key = gbfr_hash(&format!("PL{}", suffix)); // uppercase == record key
            let character_type = gbfr_hash(&format!("Pl{}", suffix)); // == actor type-id
            map.insert(key, character_type);
        }
        map
    })
}

pub(crate) fn character_type_for_key(key: u32) -> Option<u32> {
    key_to_character().get(&key).copied()
}

static DEATH_COUNTS: OnceLock<Mutex<HashMap<(u32, Option<u8>), u32>>> = OnceLock::new();

pub(crate) fn bump_death_count(player_key: u32, slot: Option<u8>) -> u32 {
    let counts = DEATH_COUNTS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut counts = match counts.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let counter = counts.entry((player_key, slot)).or_insert(0);
    *counter += 1;
    *counter
}

fn clear_death_counts() {
    let Some(counts) = DEATH_COUNTS.get() else {
        return;
    };
    let mut counts = match counts.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    counts.clear();
}

pub(crate) enum Boundary {
    QuestLoad,
    RepeatRun,
    /// One area of a Conflux survey ended. The encounter continues, because a
    /// whole survey is one log.
    ConfluxArea,
}

pub(crate) fn on_encounter_boundary(boundary: Boundary) {
    super::damage::clear_state_transitions();
    super::damage::clear_action_owners();
    super::damage::clear_replicated_sources();
    clear_identity_owned(boundary);
}

fn clear_identity_owned(boundary: Boundary) {
    if !matches!(boundary, Boundary::ConfluxArea) {
        clear_death_counts();
    }
    if matches!(boundary, Boundary::QuestLoad) {
        clear_identity_and_roster();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static SERIAL: Mutex<()> = Mutex::new(());

    fn serial() -> std::sync::MutexGuard<'static, ()> {
        SERIAL.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn death_counts_reset_on_both_boundaries() {
        let _serial = serial();
        let key = 0xDEAD_5000;
        clear_death_counts();
        assert_eq!(bump_death_count(key, Some(0)), 1);
        assert_eq!(bump_death_count(key, Some(0)), 2);
        assert_eq!(bump_death_count(key, Some(1)), 1);
        assert_eq!(bump_death_count(key, Some(1)), 2);
        assert_eq!(bump_death_count(key, None), 1);

        clear_identity_owned(Boundary::RepeatRun);
        assert_eq!(bump_death_count(key, Some(0)), 1);
        clear_identity_owned(Boundary::QuestLoad);
        assert_eq!(bump_death_count(key, Some(0)), 1);
    }

    #[test]
    fn conflux_area_boundary_keeps_death_counts_and_identities() {
        let _serial = serial();
        let key = 0xDEAD_6000;
        let record = 0xBEEF_0002usize;
        clear_death_counts();
        identity_store()
            .lock()
            .unwrap()
            .insert((0, record), PlayerIdentity::default());

        assert_eq!(bump_death_count(key, Some(0)), 1);
        assert_eq!(bump_death_count(key, Some(0)), 2);

        clear_identity_owned(Boundary::ConfluxArea);

        assert_eq!(bump_death_count(key, Some(0)), 3);
        assert!(identity_store().lock().unwrap().contains_key(&(0, record)));

        clear_identity_owned(Boundary::RepeatRun);
        assert_eq!(bump_death_count(key, Some(0)), 1);
        clear_identity_owned(Boundary::QuestLoad);
    }

    #[test]
    fn quest_load_clears_identity_state_repeat_preserves_it() {
        let _serial = serial();
        let record = 0xBEEF_0001usize;
        identity_store()
            .lock()
            .unwrap()
            .insert((0, record), PlayerIdentity::default());
        last_roster_sig().lock().unwrap().clear();
        last_roster_sig().lock().unwrap().push((0, 0x1234, vec![1]));

        clear_identity_owned(Boundary::RepeatRun);
        assert!(identity_store().lock().unwrap().contains_key(&(0, record)));
        assert!(!last_roster_sig().lock().unwrap().is_empty());

        clear_identity_owned(Boundary::QuestLoad);
        assert!(identity_store().lock().unwrap().is_empty());
        assert!(last_roster_sig().lock().unwrap().is_empty());
    }

    #[test]
    fn network_user_store_survives_both_boundaries() {
        let _serial = serial();
        set_network_user(2, NetworkUser::default());
        set_local_network_user(NetworkUser::default());

        clear_identity_owned(Boundary::RepeatRun);
        clear_identity_owned(Boundary::QuestLoad);

        assert!(network_user_for_enw(2).is_some());
        assert!(local_network_user().is_some());
    }

    #[test]
    fn reset_announced_forgets_prior_announcements() {
        let _serial = serial();
        announced_identities().lock().unwrap().insert(0xA11CE, 1);
        assert_eq!(announced_identities().lock().unwrap().get(&0xA11CE), Some(&1));

        reset_announced();
        assert!(announced_identities().lock().unwrap().get(&0xA11CE).is_none());

        announced_identities().lock().unwrap().insert(0xB0B, 0);
        clear_identity_owned(Boundary::RepeatRun);
        assert_eq!(announced_identities().lock().unwrap().get(&0xB0B), Some(&0));

        clear_identity_owned(Boundary::QuestLoad);
        assert!(announced_identities().lock().unwrap().get(&0xB0B).is_none());
    }
}
