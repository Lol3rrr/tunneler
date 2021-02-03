use crate::{Connection, Connections, Message, MessageHeader, MessageType};

use log::error;

pub async fn respond(
    id: u32,
    send_queue: tokio::sync::mpsc::UnboundedSender<Message>,
    proxied_con: std::sync::Arc<Connection>,
    users: std::sync::Arc<Connections<Connection>>,
) {
    loop {
        let mut buf = vec![0; 4092];
        match proxied_con.read(&mut buf).await {
            Ok(0) => {
                proxied_con.close();
                users.remove(id);

                return;
            }
            Ok(n) => {
                let header = MessageHeader::new(id, MessageType::Data, n as u64);
                let msg = Message::new(header, buf);
                match send_queue.send(msg) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("[{}][Server] Forwarding Data: {}", id, e);
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
