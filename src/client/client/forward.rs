use crate::Message;

use log::{debug, error};
use tokio::io::AsyncWriteExt;

pub async fn forward(
    mut write_user_con: tokio::io::WriteHalf<tokio::net::TcpStream>,
    mut receive_queue: tokio::sync::broadcast::Receiver<Message>,
) {
    loop {
        let message = match receive_queue.recv().await {
            Ok(msg) => msg,
            Err(e) => {
                error!("Reading from Queue: {}", e);
                return;
            }
        };

        let data = message.get_data();
        debug!(
            "[{}][Proxied] Sent {} Bytes",
            message.get_header().get_id(),
            data.len()
        );
        match write_user_con.write_all(&data).await {
            Ok(_) => {}
            Err(e) => {
                error!("Sending to User-con: {}", e);
                return;
            }
        };
    }
}