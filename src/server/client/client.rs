use crate::server::client::ClientManager;
use crate::{Connection, Connections};
use crate::{Message, MessageHeader, MessageType};

use log::{debug, error};

#[derive(Clone)]
pub struct Client {
    id: u32,
    con: std::sync::Arc<Connection>,
    user_cons: std::sync::Arc<Connections<std::sync::Arc<Connection>>>,
    client_manager: std::sync::Arc<ClientManager>,
    send_queue: tokio::sync::mpsc::UnboundedSender<Message>,
}

impl Client {
    pub fn new(
        id: u32,
        con: std::sync::Arc<Connection>,
        client_manager: std::sync::Arc<ClientManager>,
        send_queue: tokio::sync::mpsc::UnboundedSender<Message>,
    ) -> Client {
        Client {
            id,
            con,
            user_cons: std::sync::Arc::new(Connections::new()),
            client_manager,
            send_queue,
        }
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    async fn drain(&self, length: u64) {
        match self.con.drain(length as usize).await {
            Ok(_) => {}
            Err(e) => {
                error!("[{}] Draining Connection: {}", self.get_id(), e);
            }
        };
    }

    pub async fn read_respond(self) {
        loop {
            let mut head_buf = [0; 13];
            let header = match self.con.read_total(&mut head_buf, 13).await {
                Ok(_) => {
                    let h = MessageHeader::deserialize(head_buf);
                    if h.is_none() {
                        error!("[{}] Deserializing Header: {:?}", self.get_id(), head_buf);
                        continue;
                    }
                    h.unwrap()
                }
                Err(e) => {
                    error!("[{}] Reading from Client-Connection: {}", self.get_id(), e);
                    return;
                }
            };

            if let MessageType::Heartbeat = header.get_kind() {
                debug!("[{}] Received Heartbeat", self.id);
                continue;
            }

            match header.get_kind() {
                MessageType::Data => {}
                _ => {
                    error!(
                        "[{}][{}] Unexpected Operation: {:?}",
                        self.get_id(),
                        header.get_id(),
                        header.get_kind()
                    );
                    self.drain(header.get_length()).await;
                }
            };

            // Forwarding the message to the actual user
            let user_con = match self.user_cons.get(header.get_id()) {
                Some(s) => s,
                None => {
                    error!(
                        "[{}] No Connection found with ID: {}",
                        self.get_id(),
                        header.get_id()
                    );
                    // TODO
                    // This also then needs to drain the next data that belongs to
                    // this incorrect request as this otherwise it will bring
                    // everyting else out of order as well
                    self.drain(header.get_length()).await;
                    continue;
                }
            };

            match self
                .con
                .forward_to_connection(&header, user_con.clone())
                .await
            {
                Ok(_) => {}
                Err(e) => {
                    error!(
                        "[{}][{}] Forwarding to User-Connection: {}",
                        self.get_id(),
                        header.get_id(),
                        e
                    );
                }
            };
        }
    }

    async fn close_user_connection(client: Self, id: u32) {
        client.user_cons.remove(id);

        let header = MessageHeader::new(id, MessageType::Close, 0);
        let msg = Message::new(header, vec![0]);
        match client.send_queue.send(msg) {
            Ok(_) => {}
            Err(e) => {
                error!("[{}][{}] Sending Close Message: {}", client.get_id(), id, e);
            }
        };
    }

    fn close(&self) {
        debug!("[{}] Closing Connection", self.get_id());
        self.client_manager.remove_con(self.get_id());
    }

    pub async fn sender(self, mut queue: tokio::sync::mpsc::UnboundedReceiver<Message>) {
        loop {
            let msg = match queue.recv().await {
                Some(m) => m,
                None => {
                    error!("[{}][Sender] Receiving Message from Queue", self.get_id());
                    return;
                }
            };

            debug!("[{}][Sender] Got message to send", self.get_id());

            let data = msg.serialize();
            let total_data_length = data.len();
            match self.con.write_total(&data, total_data_length).await {
                Ok(_) => {
                    debug!(
                        "[{}][Sender] Send {} out bytes",
                        self.get_id(),
                        total_data_length
                    );
                }
                Err(e) => {
                    error!("[{}][Sender] Sending Message: {}", self.get_id(), e);
                    return;
                }
            };
        }
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
            match con.read_raw(&mut buf).await {
                Ok(0) => {
                    break;
                }
                Ok(n) => {
                    // Package the Users-Data in a new custom-message
                    let header = MessageHeader::new(id, MessageType::Data, n as u64);
                    let msg = Message::new(header, buf);

                    // Puts the message in the queue to be send to the client
                    match client.send_queue.send(msg) {
                        Ok(_) => {}
                        Err(e) => {
                            error!(
                                "[{}][{}] Forwarding message to client: {}",
                                client.get_id(),
                                id,
                                e
                            );
                        }
                    };
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    error!("[{}][{}] Reading from User-Con: {}", client.get_id(), id, e);
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
