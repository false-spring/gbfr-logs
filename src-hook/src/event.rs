use tokio::sync::broadcast;

use protocol::Message;

pub type Tx = broadcast::Sender<Message>;
pub type Rx = broadcast::Receiver<Message>;
