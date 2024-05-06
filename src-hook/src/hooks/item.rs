use anyhow::{anyhow, Result};

use protocol::{ItemGiveEvent, Message};
use retour::static_detour;

use crate::{event, process::Process};

type OnGiveItemFunc = unsafe extern "system" fn(*const usize, u32, i32, u8) -> usize;

static_detour! {
    static OnGiveItem: unsafe extern "system" fn(*const usize, u32, i32, u8) -> usize;
}

const ON_GIVE_ITEM_SIG: &str = "E8 $ { ' } 8B 17 4C 89 E1 E8";

#[derive(Clone)]
pub struct OnItemGiveHook {
    tx: event::Tx,
}

impl OnItemGiveHook {
    pub fn new(tx: event::Tx) -> Self {
        OnItemGiveHook { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let cloned_self = self.clone();

        if let Ok(on_item_give) = process.search_address(ON_GIVE_ITEM_SIG) {
            #[cfg(feature = "console")]
            println!("Found on item give");

            unsafe {
                let func: OnGiveItemFunc = std::mem::transmute(on_item_give);
                OnGiveItem
                    .initialize(func, move |a1, a2, a3, a4| cloned_self.run(a1, a2, a3, a4))?;
                OnGiveItem.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find on_item_give"));
        }

        Ok(())
    }

    fn run(&self, a1: *const usize, item_id: u32, count: i32, flag: u8) -> usize {
        #[cfg(feature = "console")]
        println!(
            "on item give, item_id={:x}, count={}, flag={}",
            item_id, count, flag
        );

        let ret = unsafe { OnGiveItem.call(a1, item_id, count, flag) };

        let _ = self
            .tx
            .send(Message::ItemGiveEvent(ItemGiveEvent { item_id, count }));

        ret
    }
}
