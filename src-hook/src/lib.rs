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

#[tokio::main]
async fn setup() {
    info!("Setting up hooks...");

    match hook::init() {
        Ok(_) => info!("Hooks initialized"),
        Err(e) => warn!("Error initializing hooks: {:?}", e),
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
