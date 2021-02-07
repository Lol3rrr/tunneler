use crate::pool;
use crate::streams::{mpsc, RecvError};
use crate::Message;

use log::error;
use tokio::io::AsyncWriteExt;

pub async fn forward(
    mut raw_write_user_con: pool::connection::Connection<tokio::net::tcp::OwnedWriteHalf>,
    mut receive_queue: mpsc::StreamReader<Message>,
) {
    let write_user_con = raw_write_user_con.as_mut();
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
