use crate::server::client::ClientManager;
use crate::{Connection, Connections};
use crate::{Message, MessageHeader, MessageType};

#[derive(Clone)]
pub struct Client {
    id: u32,
    con: std::sync::Arc<Connection>,
    user_cons: std::sync::Arc<Connections<Connection>>,
    client_manager: std::sync::Arc<ClientManager>,
}

impl Client {
    pub fn new(
        id: u32,
        con: std::sync::Arc<Connection>,
        client_manager: std::sync::Arc<ClientManager>,
    ) -> Client {
        let connections: std::sync::Arc<Connections<Connection>> =
            std::sync::Arc::new(Connections::new());

        Client {
            id,
            con,
            user_cons: connections,
            client_manager,
        }
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub async fn read_respond(self) {
        loop {
            let mut head_buf = [0; 13];
            let header = match self.con.read(&mut head_buf).await {
                Ok(0) => {
                    self.close();
                    return;
                }
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

            if let MessageType::Heartbeat = header.get_kind() {
                println!("[{}] Received Heartbeat", self.id);
                continue;
            }

            let data_length = header.get_length() as usize;
            let mut to_read = data_length;
            while to_read > 0 {
                let mut read_buf = vec![0; to_read];
                match self.con.read(&mut read_buf).await {
                    Ok(0) => {
                        self.close();
                        return;
                    }
                    Ok(n) => {
                        to_read -= n;

                        let user_con = match self.user_cons.get(header.get_id()) {
                            Some(s) => s,
                            None => {
                                println!("No client found with ID: {}", header.get_id());
                                continue;
                            }
                        };

                        match header.get_kind() {
                            MessageType::Data => {
                                match user_con.write(&read_buf[0..n]).await {
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
                        };
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        continue;
                    }
                    Err(e) => {
                        println!("Reading client: {}", e);
                        return;
                    }
                }
            }
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

    fn close(&self) {
        println!("Closing Client-Connection");
        self.client_manager.remove_con(self.get_id());
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
                            client.close();
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
