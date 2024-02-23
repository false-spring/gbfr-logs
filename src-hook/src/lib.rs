use std::ffi::OsStr;
use std::path::PathBuf;

use anyhow::Context;
use interprocess::os::windows::named_pipe::tokio::MsgWriterPipeStream;
use interprocess::os::windows::named_pipe::{
    tokio::PipeListenerOptionsExt, PipeListenerOptions, PipeMode,
};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

mod hook;
mod process;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Actor {
    actor_type: String,
    actor_idx: u32,
    actor_id: u32,
    party_idx: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum Message {
    Hello,
    DamageEvent {
        source: Actor,
        target: Actor,
        damage: i32,
        flags: u64,
        action_id: i32,
    },
}

type Tx = broadcast::Sender<Message>;
type Rx = broadcast::Receiver<Message>;

const PIPE_NAME: &str = r"\\.\pipe\gbfr-logs";

fn handle_client(_stream: MsgWriterPipeStream, _rx: Rx) {}

#[derive(Debug)]
struct Server {
    tx: Tx,
}

impl Server {
    fn new() -> Self {
        let (tx, _) = broadcast::channel::<Message>(1024);
        Server { tx }
    }

    async fn run(&self) {
        if let Ok(listener) = PipeListenerOptions::new()
            .name(OsStr::new(PIPE_NAME))
            .mode(PipeMode::Messages)
            .accept_remote(false)
            .create_tokio::<MsgWriterPipeStream>()
        {
            loop {
                let read_pipe = listener.accept().await;
                match read_pipe {
                    Ok(stream) => {
                        let rx = self.tx.subscribe();
                        tokio::spawn(async move {
                            handle_client(stream, rx);
                        });
                    }
                    Err(e) => {
                        warn!("Error accepting client: {:?}", e);
                    }
                }
            }
        }
    }
}

const PROCESS_DAMAGE_EVENT_SIG: &str = "e8 $ { ' } 66 83 bc 24 ? ? ? ? ?";
const PROCESS_DOT_EVENT_SIG: &str = "44 89 74 24 ? 48 ? ? ? ? 48 ? ? e8 $ { ' } 4c";
const ON_ENTER_AREA_SIG: &str = "e8 $ { ' } c5 ? ? ? c5 f8 29 45 ? c7 45 ? ? ? ? ?";
const P_QWORD_1467572B0_SIG: &str = "48 ? ? $ { ' } 83 66 ? ? 48 ? ?";

#[tokio::main]
async fn setup() {
    info!("Scanning for patterns");

    // See https://github.com/nyaoouo/GBFR-ACT/blob/5801c193de2f474764b55b7c6b759c3901dc591c/injector.py#L1773-L1809
    if let Ok(_process_dmg_evt) = hook::search(PROCESS_DAMAGE_EVENT_SIG) {
        info!("Found process_dmg_evt");
    } else {
        warn!("Could not find process_dmg_evt");
    }
    if let Ok(_process_dot_evt) = hook::search(PROCESS_DOT_EVENT_SIG) {
        info!("Found process_dot_evt")
    } else {
        warn!("Could not find process_dot_evt");
    }
    if let Ok(_on_enter_area) = hook::search(ON_ENTER_AREA_SIG) {
        info!("Found on_enter_area");
    } else {
        warn!("Could not find on_enter_area");
    }
    #[allow(non_snake_case)]
    if let Ok(_p_qword_1467572B0) = hook::search(P_QWORD_1467572B0_SIG) {
        info!("Found p_qword_1467572B0");
    } else {
        warn!("Could not find p_qword_1467572B0");
    }

    info!("Setting up named pipe listener");

    let server = Server::new();
    let tx = server.tx.clone();

    tokio::spawn(async move {
        server.run().await;
    });

    tx.send(Message::Hello).unwrap();

    info!("Exiting");
}

fn initialize_logger() -> anyhow::Result<()> {
    let application_data_dir = dirs::data_dir().context("Could not find data folder")?;
    let mut log_file = PathBuf::new();

    log_file.push(application_data_dir);
    log_file.push("gbfr-logs");
    std::fs::create_dir_all(log_file.as_path())?;
    log_file.push("gbfr-logs.txt");

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {}] {}",
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(fern::log_file(log_file)?)
        .apply()?;

    Ok(())
}

#[ctor::ctor]
fn entry() {
    let _ = initialize_logger();
    std::thread::spawn(setup);
}
