use crate::{Connections, Message, MessageHeader, MessageType};

use log::{debug, error};
use tokio::io::AsyncReadExt;

pub async fn respond(
    id: u32,
    send_queue: tokio::sync::mpsc::UnboundedSender<Message>,
    mut read_user_con: tokio::io::ReadHalf<tokio::net::TcpStream>,
    users: std::sync::Arc<Connections<tokio::sync::broadcast::Sender<Message>>>,
) {
    loop {
        let mut buf = vec![0; 4092];
        match read_user_con.read(&mut buf).await {
            Ok(0) => {
                debug!("[{}][Proxied] Read 0 Bytes", id);
                debug!("[{}][Proxied] Closing connection", id);
                //users.remove(id);

                //return;
            }
            Ok(n) => {
                let header = MessageHeader::new(id, MessageType::Data, n as u64);
                let msg = Message::new(header, buf);
                match send_queue.send(msg) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("[{}][Proxied] Adding Data to Server-Queue: {}", id, e);
                    }
                };
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                error!("[{}][Proxied] Reading from proxied: {}", id, e);
                return;
            }
        };
    }
}
