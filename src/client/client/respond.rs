use crate::pool;
use crate::streams::mpsc;
use crate::{Connections, Message, MessageHeader, MessageType};

use log::error;
use tokio::io::AsyncReadExt;

pub async fn respond(
    id: u32,
    send_queue: tokio::sync::mpsc::UnboundedSender<Message>,
    mut raw_read_user_con: pool::connection::Connection<tokio::net::tcp::OwnedReadHalf>,
    users: std::sync::Arc<Connections<mpsc::StreamWriter<Message>>>,
) {
    let read_user_con = raw_read_user_con.as_mut();
    loop {
        let mut buf = vec![0; 4092];
        match read_user_con.read(&mut buf).await {
            Ok(0) => {
                break;
            }
            Ok(n) => {
                let header = MessageHeader::new(id, MessageType::Data, n as u64);
                let msg = Message::new(header, buf);
                match send_queue.send(msg) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("[{}] Adding Data to Queue: {}", id, e);
                    }
                };
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                error!("[{}] Reading: {}", id, e);
                break;
            }
        };
    }

    users.remove(id);
    let header = MessageHeader::new(id, MessageType::Close, 0);
    let msg = Message::new(header, vec![]);
    match send_queue.send(msg) {
        Ok(_) => {}
        Err(e) => {
            error!("[{}] Adding Close message to Server-Queue: {}", id, e);
        }
    };

    raw_read_user_con.invalidate();
}
