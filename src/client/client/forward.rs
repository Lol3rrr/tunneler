use tunneler_core::client::{user_con::OwnedReceiver, Receiver};

use log::error;
use tokio::io::AsyncWriteExt;

pub async fn forward(
    mut write_user_con: tokio::net::tcp::OwnedWriteHalf,
    mut receive_queue: OwnedReceiver,
) {
    loop {
        let message = match receive_queue.recv_msg().await {
            Ok(msg) => msg,
            Err(e) => {
                error!("Reading from Queue: {}", e);
                return;
            }
        };

        let data = message.get_data();
        match write_user_con.write_all(data).await {
            Ok(_) => {}
            Err(e) => {
                error!("Sending to User-con: {}", e);
                return;
            }
        };
    }
}
