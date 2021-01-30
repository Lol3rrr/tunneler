use crate::{Connection, Connections};
use crate::{Message, MessageHeader, MessageType};

#[derive(Clone)]
pub struct Client {
    con: std::sync::Arc<Connection>,
    user_cons: std::sync::Arc<Connections<Connection>>,
}

impl Client {
    pub fn new(con: std::sync::Arc<Connection>) -> Client {
        let connections: std::sync::Arc<Connections<Connection>> =
            std::sync::Arc::new(Connections::new());

        Client {
            con,
            user_cons: connections,
        }
    }

    pub async fn read_respond(self) {
        loop {
            let mut head_buf = [0; 13];
            let header = match self.con.read(&mut head_buf).await {
                Ok(0) => continue,
                Ok(_) => {
                    let h = MessageHeader::deserialize(head_buf);
                    if h.is_none() {
                        println!("Deserializing Header: {:?}", head_buf);
                        continue;
                    }
                    h.unwrap()
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    println!("[Error][Server] Reading from Req: {}", e);
                    continue;
                }
            };

            let data_length = header.get_length() as usize;
            let mut buf = vec![0; data_length];
            match self.con.read(&mut buf).await {
                Ok(0) => {
                    break;
                }
                Ok(n) => {
                    if n != data_length {
                        println!(
                            "Read bytes is different to the data-length: {} != {}",
                            n, data_length
                        );
                    }

                    match self.user_cons.get(header.get_id()) {
                        Some(s) => match header.get_kind() {
                            MessageType::Data => {
                                match s.write(&buf).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        println!("Forwarding: {}", e);
                                    }
                                };
                            }
                            _ => {
                                println!(
                                    "[{}] Unknown operation: {:?}",
                                    header.get_id(),
                                    header.get_kind()
                                );
                            }
                        },
                        None => {
                            println!("No client found for this");
                        }
                    };
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    println!("Reading client: {}", e);
                    return;
                }
            };
        }
    }

    async fn close_user_connection(client: Self, id: u32) {
        client.user_cons.remove(id);

        let header = MessageHeader::new(id, MessageType::Close, 0);
        let close_msg = Message::new(header, vec![0]);
        match client.con.write(&close_msg.serialize()).await {
            Ok(_) => {}
            Err(e) => {
                println!("Sending Close Message: {}", e);
            }
        };
    }

    /// Process a new user connection
    ///
    /// Params:
    /// * client: The Server-Client to use
    /// * id: The ID of the user-connection
    /// * con: The User-Connection
    async fn process_connection(client: Self, id: u32, con: std::sync::Arc<Connection>) {
        // Reads and forwards all the data from the socket to the client
        loop {
            let mut buf = vec![0; 4092];
            // Try to read data from the user
            //
            // this may still fail with `WouldBlock` if the readiness event is
            // a false positive.
            match con.read(&mut buf).await {
                Ok(0) => {
                    break;
                }
                Ok(n) => {
                    // Package the Users-Data in a new custom-message
                    let header = MessageHeader::new(id, MessageType::Data, n as u64);
                    let msg = Message::new(header, buf);

                    // Send the custom message over the client connection to the client
                    match client.con.write(&msg.serialize()).await {
                        Ok(_) => {}
                        Err(e) => {
                            println!("Sending: {}", e);
                            println!("Needs to close the client connection");
                            return;
                        }
                    };
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    println!("Reading from Req: {}", e);
                    println!("{:?}", e.kind());
                    if e.kind() == std::io::ErrorKind::ConnectionReset {
                        tokio::task::spawn(Client::close_user_connection(client, id));
                    }
                    return;
                }
            }
        }
    }

    /// Adds a new user connection to this server-client
    ///
    /// Params:
    /// * id: The ID of the new user connection
    /// * con: The new user connection
    pub fn new_con(&self, id: u32, con: std::sync::Arc<Connection>) {
        self.user_cons.set(id, con.clone());

        tokio::task::spawn(Self::process_connection(self.clone(), id, con));
    }
}
