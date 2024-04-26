use std::ffi::OsStr;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use futures::io::AsyncWriteExt;
use interprocess::os::windows::named_pipe::tokio::{ByteWriterPipeStream, PipeListenerOptionsExt};
use interprocess::os::windows::named_pipe::{PipeListenerOptions, PipeMode};
use log::{info, warn};
use tokio::sync::broadcast;

mod event;
mod hooks;
mod process;

use protocol::Message;

async fn handle_client(mut stream: ByteWriterPipeStream, mut rx: event::Rx) -> Result<()> {
    while let Ok(msg) = rx.recv().await {
        let bytes = protocol::bincode::serialize(&msg)?;
        stream.write_all(&bytes).await?;
    }

    Ok(())
}

#[derive(Debug)]
struct Server {
    tx: event::Tx,
}

impl Server {
    fn new() -> Self {
        let (tx, _) = broadcast::channel::<Message>(1024);
        Server { tx }
    }

    async fn run(&self) {
        if let Ok(listener) = PipeListenerOptions::new()
            .name(OsStr::new(protocol::PIPE_NAME))
            .mode(PipeMode::Bytes)
            .accept_remote(false)
            .create_tokio::<ByteWriterPipeStream>()
        {
            loop {
                let read_pipe = listener.accept().await;
                match read_pipe {
                    Ok(stream) => {
                        let rx = self.tx.subscribe();
                        tokio::spawn(async move {
                            let _ = handle_client(stream, rx).await;
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
    info!("Setting up named pipe listener");

    let server = Server::new();
    let tx = server.tx.clone();

    info!("Setting up hooks...");

    match hooks::setup_hooks(tx) {
        Ok(_) => info!("Hooks initialized"),
        Err(e) => warn!("Error initializing hooks: {:?}", e),
    }

    #[cfg(feature = "console")]
    println!("Hook library initialized");

    let _ = std::io::stdout().flush();

    server.run().await;
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
    #[cfg(feature = "console")]
    unsafe {
        let _ = windows::Win32::System::Console::AllocConsole();
    }

    let _ = initialize_logger();
    std::thread::spawn(setup);
}
