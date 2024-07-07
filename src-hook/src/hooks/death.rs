use anyhow::{anyhow, Result};
use retour::static_detour;

use crate::{event, process::Process};

type DeathEventFunc = unsafe extern "system" fn(*const usize) -> usize;

static_detour! {
    static OnDeathEvent: unsafe extern "system" fn(*const usize) -> usize;
}

const ON_DEATH_EVENT_SIG: &str = "e8 $ { ' } 49 ? ? 48 ? ? ? ? ? ? 83 78 ? ?";

#[derive(Clone)]
pub struct OnDeathHook {
    tx: event::Tx,
}

impl OnDeathHook {
    pub fn new(tx: event::Tx) -> Self {
        Self { tx }
    }

    pub fn setup(&self, process: &Process) -> Result<()> {
        let cloned_self = self.clone();

        if let Ok(on_death_event) = process.search_address(ON_DEATH_EVENT_SIG) {
            #[cfg(feature = "console")]
            println!("Found on death event");

            unsafe {
                let func: DeathEventFunc = std::mem::transmute(on_death_event);
                OnDeathEvent.initialize(func, move |a1| cloned_self.run(a1))?;
                OnDeathEvent.enable()?;
            }
        } else {
            return Err(anyhow!("Could not find on_death_event"));
        }

        Ok(())
    }

    fn run(&self, a1: *const usize) -> usize {
        #[cfg(feature = "console")]
        println!("on death");

        let ret = unsafe { OnDeathEvent.call(a1) };

        let entity_ptr = unsafe { a1.byte_add(0x10).read() as *const usize };
        let actor_index = unsafe { entity_ptr.byte_add(0x170).read() } as u32;
        let death_counter = unsafe { a1.byte_add(0xEC).read() } as u32;

        let event = protocol::Message::OnDeathEvent(protocol::OnDeathEvent {
            actor_index,
            death_counter,
        });

        let _ = self.tx.send(event);

        ret
    }
}
