//! Online-party identity capture, keyed by eNwPlayerId. That id is an
//! arbitrary permutation of party_index, not the party index itself.

use anyhow::{anyhow, Result};
use retour::static_detour;

use crate::{
    hooks::{player_directory, safe_read},
    process::Process,
};

const OBJ_NAME_OFFSET: usize = 0x18;
const OBJ_ENTITY_ID_OFFSET: usize = 0x38;
/// eNwPlayerId; ctor-init -1, > 3 means no slot yet.
const OBJ_SLOT_OFFSET: usize = 0x68;
const OBJ_IS_LOCAL_OFFSET: usize = 0x70;

const STR_SIZE_OFFSET: usize = 0x10;
const STR_CAP_OFFSET: usize = 0x18;
const STR_SSO_CAP: usize = 15;
const MAX_STRING_LEN: usize = 128;

const ON_POPULATE_NETWORK_USER_SIG: &str =
    "41 57 41 56 41 55 41 54 56 57 55 53 48 83 ec 58 48 89 d7 48 89 ce 80 79 70 00";

#[derive(Clone, Default)]
pub(crate) struct NetworkUser {
    pub name: Option<String>,
    pub entity_id: Option<String>,
}

fn read_msvc_string(base: *const u8) -> Option<String> {
    let size = safe_read::<usize>(base.wrapping_add(STR_SIZE_OFFSET) as *const usize)?;
    let cap = safe_read::<usize>(base.wrapping_add(STR_CAP_OFFSET) as *const usize)?;
    if size == 0 || size > MAX_STRING_LEN || cap > 0x1_0000 || size > cap {
        return None;
    }

    let data = if cap <= STR_SSO_CAP {
        base
    } else {
        match safe_read::<*const u8>(base as *const *const u8) {
            Some(ptr) if !ptr.is_null() => ptr,
            _ => return None,
        }
    };

    // A short allocation can fault the chunk over-read; the per-byte loop is
    // the fallback.
    let bytes = if let Some(chunk) =
        safe_read::<[u8; MAX_STRING_LEN]>(data as *const [u8; MAX_STRING_LEN])
    {
        let mut v = chunk[..size].to_vec();
        if let Some(nul) = v.iter().position(|&b| b == 0) {
            v.truncate(nul);
        }
        v
    } else {
        let mut v = Vec::with_capacity(size);
        for i in 0..size {
            let byte = safe_read::<u8>(data.wrapping_add(i))?;
            if byte == 0 {
                break;
            }
            v.push(byte);
        }
        v
    };

    if bytes.is_empty() {
        None
    } else {
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }
}

type OnPopulateNetworkUserFunc =
    unsafe extern "system" fn(*const usize, *const usize) -> usize;

static_detour! {
    static OnPopulateNetworkUser: unsafe extern "system" fn(*const usize, *const usize) -> usize;
}

#[derive(Clone)]
pub struct OnPopulateNetworkUserHook;

impl OnPopulateNetworkUserHook {
    pub fn new() -> Self {
        OnPopulateNetworkUserHook
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let cloned_self = self.clone();

        if let Ok(address) = process.search_address_start(ON_POPULATE_NETWORK_USER_SIG) {
            #[cfg(feature = "console")]
            println!("Found populate network user");

            unsafe {
                let func: OnPopulateNetworkUserFunc = std::mem::transmute(address);
                OnPopulateNetworkUser
                    .initialize(func, move |obj, source| cloned_self.run(obj, source))?;
                OnPopulateNetworkUser.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find populate_network_user"));
        }

        Ok(())
    }

    fn run(&self, obj: *const usize, source: *const usize) -> usize {
        let ret = unsafe { OnPopulateNetworkUser.call(obj, source) };

        let base = obj as *const u8;

        if let Some(enw) = safe_read::<u32>(base.wrapping_add(OBJ_SLOT_OFFSET) as *const u32) {
            let name = read_msvc_string(base.wrapping_add(OBJ_NAME_OFFSET));
            let entity_id = read_msvc_string(base.wrapping_add(OBJ_ENTITY_ID_OFFSET));
            let is_local =
                safe_read::<u8>(base.wrapping_add(OBJ_IS_LOCAL_OFFSET)).unwrap_or(0) != 0;

            #[cfg(feature = "console")]
            println!(
                "network user: eNw={} name={:?} id={:?} local={}",
                enw as i32, name, entity_id, is_local
            );

            let user = NetworkUser { name, entity_id };

            // The local object's eNw reads -1 until the lobby roster pass assigns it.
            if is_local {
                player_directory::set_local_network_user(user.clone());
            }

            if enw <= 3 {
                player_directory::set_network_user(enw as u8, user);
            }
        }

        ret
    }
}
