use crate::{Connection, Connections, Destination, Error, Message, MessageHeader, MessageType};

use log::{debug, error};

mod read_forward;

pub async fn handle_connection(
    server_con: std::sync::Arc<Connection>,
    send_queue: tokio::sync::mpsc::UnboundedSender<Message>,
    outgoing: std::sync::Arc<Connections<tokio::sync::broadcast::Sender<Message>>>,
    out_dest: &Destination,
) -> Result<(), Error> {
    loop {
        let mut head_buf = [0; 13];
        let header = match server_con.read(&mut head_buf).await {
            Ok(0) => {
                return Err(Error::from(std::io::Error::from(
                    std::io::ErrorKind::ConnectionReset,
                )));
            }
            Ok(_) => {
                let h = MessageHeader::deserialize(head_buf);
                if h.is_none() {
                    error!("Deserializing Header: {:?}", head_buf);
                    continue;
                }
                h.unwrap()
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                error!("[Server] Reading from Req: {}", e);
                return Err(Error::from(e));
            }
        };

        match header.get_kind() {
            MessageType::Close => {
                debug!("[Server] Close Connection: {}", header.get_id());
                close_user_connection::close_user_connection(header.get_id(), &outgoing);
                continue;
            }
            MessageType::Data => {}
            _ => {
                debug!(
                    "[Server][{}] Unknown Operation: {:?}",
                    header.get_id(),
                    header.get_kind()
                );
                continue;
            }
        };

        let data_length = header.get_length() as usize;
        let mut buf = vec![0; data_length];
        // Try to read data, this may still fail with `WouldBlock`
        // if the readiness event is a false positive.
        match server_con.read(&mut buf).await {
            Ok(0) => continue,
            Ok(n) => {
                if n != data_length {
                    debug!(
                        "Read bytes doesnt match body length: {} != {}",
                        n, data_length
                    );
                }

                let msg = Message::new(header, buf);
                read_forward::read_forward(send_queue.clone(), outgoing.clone(), out_dest, msg)
                    .await;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                error!("[Server] Reading from Req: {}", e);
                continue;
            }
        };
    }
}
