use anyhow::{anyhow, Result};
use retour::static_detour;

use crate::{event, process::Process};

use super::{
    actor_type_id, get_source_parent, probe_actor, safe_read, ENTITY_SPECIFIED_INSTANCE_OFFSET,
};

type HealSinkFunc = unsafe extern "system" fn(*const usize, *const usize) -> usize;

static_detour! {
    static OnHeal: unsafe extern "system" fn(*const usize, *const usize) -> usize;
}

const ON_HEAL_SIG: &str = "41 57 41 56 41 55 41 54 56 57 55 53 48 81 ec 98 00 00 00 \
                           c5 f8 29 b4 24 80 00 00 00 48 89 d3 48 89 cf 83 3a 04";

const HEAL_INFO_TYPE_OFFSET: usize = 0x00;
const HEAL_INFO_AMOUNT_OFFSET: usize = 0x04;
const HEAL_INFO_CHANNEL_OFFSET: usize = 0x0C;
/// The `CEntityInfo*` in the source handle triple `{u32 idx, ptr, u32 gen}` at +0x18.
const HEAL_INFO_SOURCE_INFO_OFFSET: usize = 0x20;

const HEAL_LEDGER_BEGIN_OFFSET: usize = 0x1C280;
const HEAL_LEDGER_END_OFFSET: usize = 0x1C288;

#[derive(Clone)]
pub struct OnHealHook {
    tx: event::Tx,
}

impl OnHealHook {
    pub fn new(tx: event::Tx) -> Self {
        Self { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let cloned_self = self.clone();

        if let Ok(on_heal) = process.search_address_start(ON_HEAL_SIG) {
            #[cfg(feature = "console")]
            println!("Found heal sink");

            unsafe {
                let func: HealSinkFunc = std::mem::transmute(on_heal);
                OnHeal.initialize(func, move |target, heal_info| {
                    cloned_self.run(target, heal_info)
                })?;
                OnHeal.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find heal sink"));
        }

        Ok(())
    }

    fn run(&self, target: *const usize, heal_info: *const usize) -> usize {
        let amount = safe_read((heal_info as *const u8).wrapping_add(HEAL_INFO_AMOUNT_OFFSET) as *const i32);
        let heal_type =
            safe_read((heal_info as *const u8).wrapping_add(HEAL_INFO_TYPE_OFFSET) as *const u32);
        let channel =
            safe_read((heal_info as *const u8).wrapping_add(HEAL_INFO_CHANNEL_OFFSET) as *const u32);
        let via_apply = is_apply_primitive(target, heal_info);
        let source = resolve_source_index(heal_info);
        let target_index = resolve_actor_index(target);

        let ret = unsafe { OnHeal.call(target, heal_info) };

        if let (Some(amount), Some(target_index)) = (amount, target_index) {
            let event = protocol::Message::OnHeal(protocol::HealEvent {
                source,
                target: target_index,
                amount,
                heal_type: heal_type.unwrap_or(0),
                channel: channel.unwrap_or(0),
                via_apply,
            });
            let _ = self.tx.send(event);
        }

        ret
    }
}

fn is_apply_primitive(target: *const usize, heal_info: *const usize) -> bool {
    let begin = safe_read((target as *const u8).wrapping_add(HEAL_LEDGER_BEGIN_OFFSET) as *const usize);
    let end = safe_read((target as *const u8).wrapping_add(HEAL_LEDGER_END_OFFSET) as *const usize);
    match (begin, end) {
        (Some(b), Some(e)) if b != 0 && e >= b => {
            let h = heal_info as usize;
            !(h >= b && h < e)
        }
        _ => true,
    }
}

fn resolve_source_index(heal_info: *const usize) -> Option<u32> {
    let info = safe_read(
        (heal_info as *const u8).wrapping_add(HEAL_INFO_SOURCE_INFO_OFFSET) as *const *const u8,
    )?;
    if info.is_null() {
        return None;
    }
    let instance =
        safe_read(info.wrapping_add(ENTITY_SPECIFIED_INSTANCE_OFFSET) as *const *const usize)?;
    if instance.is_null() {
        return None;
    }
    resolve_actor_index(instance)
}

fn resolve_actor_index(instance: *const usize) -> Option<u32> {
    safe_read(instance as *const usize)?;

    let type_id = actor_type_id(instance);
    if let Some((_parent_type, parent_index)) = get_source_parent(type_id, instance) {
        return Some(parent_index);
    }
    Some(probe_actor(instance).id)
}
