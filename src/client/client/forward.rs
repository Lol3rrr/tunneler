use tunneler_core::message::Message;
use tunneler_core::streams::{error::RecvError, mpsc};

use log::error;
use tokio::io::AsyncWriteExt;

pub async fn forward(
    mut write_user_con: tokio::net::tcp::OwnedWriteHalf,
    mut receive_queue: mpsc::StreamReader<Message>,
) {
    loop {
        let message = match receive_queue.recv().await {
            Ok(msg) => msg,
            Err(e) => {
                if e != RecvError::Closed {
                    error!("Reading from Queue: {}", e);
                }
                return;
            }
        };

        let data = message.get_data();
        match write_user_con.write_all(&data).await {
            Ok(_) => {}
            Err(e) => {
                error!("Sending to User-con: {}", e);
                return;
            }
        };
    }
}
