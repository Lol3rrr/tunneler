use crate::server::client::ClientManager;
use crate::streams::spsc;
use crate::Connections;
use crate::{Message, MessageHeader, MessageType};

use log::{debug, error};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// This Client represents a single Connection a Client Instance
///
/// All User-Connections are handled by an instance of this Struct
#[derive(Clone)]
pub struct Client {
    id: u32,
    user_cons: std::sync::Arc<Connections<spsc::StreamWriter<Message>>>,
    client_manager: std::sync::Arc<ClientManager>,
    client_send_queue: tokio::sync::mpsc::UnboundedSender<Message>,
}

impl Client {
    pub fn new(
        id: u32,
        client_manager: std::sync::Arc<ClientManager>,
        send_queue: tokio::sync::mpsc::UnboundedSender<Message>,
    ) -> Client {
        Client {
            id,
            user_cons: std::sync::Arc::new(Connections::new()),
            client_manager,
            client_send_queue: send_queue,
        }
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_user_cons(&self) -> std::sync::Arc<Connections<spsc::StreamWriter<Message>>> {
        self.user_cons.clone()
    }

    async fn close_user_connection(
        user_id: u32,
        client_id: u32,
        user_cons: std::sync::Arc<Connections<spsc::StreamWriter<Message>>>,
        send_queue: tokio::sync::mpsc::UnboundedSender<Message>,
    ) {
        user_cons.remove(user_id);

        let header = MessageHeader::new(user_id, MessageType::Close, 0);
        let msg = Message::new(header, vec![0; 0]);
        match send_queue.send(msg) {
            Ok(_) => {}
            Err(e) => {
                error!("[{}][{}] Sending Close Message: {}", client_id, user_id, e);
            }
        };
    }

    /// Adds a new user connection to this server-client
    ///
    /// Params:
    /// * id: The ID of the new user connection
    /// * con: The new user connection
    pub fn new_con(&self, user_id: u32, con: tokio::net::TcpStream) {
        let (read_con, write_con) = con.into_split();
        let (tx, rx) = spsc::stream();
        self.user_cons.set(user_id, tx);

        tokio::task::spawn(Self::send_user_connection(self.id, user_id, write_con, rx));
        tokio::task::spawn(Self::recv_user_connection(
            self.id,
            user_id,
            read_con,
            self.client_send_queue.clone(),
            self.user_cons.clone(),
        ));
    }

    /// Reads messages from the Client for this User and sends them to the User
    ///
    /// Params:
    /// * client_id: The ID of the client that handles this
    /// * user_id: The ID of the User for this connection
    /// * con: The User-Connection
    /// * queue: The Queue for messages that need to be send to the user
    async fn send_user_connection(
        client_id: u32,
        user_id: u32,
        mut con: tokio::net::tcp::OwnedWriteHalf,
        mut queue: spsc::StreamReader<Message>,
    ) {
        loop {
            let msg = match queue.recv().await {
                Ok(m) => m,
                Err(e) => {
                    error!("[{}][{}] Receiving from Queue: {}", client_id, user_id, e);
                    return;
                }
            };

            let data = msg.get_data();
            match con.write_all(&data).await {
                Ok(_) => {}
                Err(e) => {
                    error!("[{}][{}] Sending to User: {}", client_id, user_id, e);
                    return;
                }
            };
        }
    }

    /// Reads from a new User-Connection and sends it to the client
    ///
    /// Params:
    /// * client: The Server-Client to use
    /// * id: The ID of the user-connection
    /// * con: The User-Connection
    /// * send_queue: The Queue for requests going out to the Client
    /// * user_cons: The User-Connections belonging to this Client
    async fn recv_user_connection(
        client_id: u32,
        user_id: u32,
        mut con: tokio::net::tcp::OwnedReadHalf,
        send_queue: tokio::sync::mpsc::UnboundedSender<Message>,
        user_cons: std::sync::Arc<Connections<spsc::StreamWriter<Message>>>,
    ) {
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
                    let header = MessageHeader::new(user_id, MessageType::Data, n as u64);
                    let msg = Message::new(header, buf);

                    // Puts the message in the queue to be send to the client
                    match send_queue.send(msg) {
                        Ok(_) => {}
                        Err(e) => {
                            error!(
                                "[{}][{}] Forwarding message to client: {}",
                                client_id, user_id, e
                            );
                        }
                    };
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    error!("[{}][{}] Reading from User-Con: {}", client_id, user_id, e);
                    if e.kind() == std::io::ErrorKind::ConnectionReset {
                        Client::close_user_connection(user_id, client_id, user_cons, send_queue)
                            .await;
                    }
                    return;
                }
            }
        }
    }

    async fn drain(read_con: &mut tokio::net::tcp::OwnedReadHalf, size: usize) {
        let mut tmp_buf = vec![0; size];
        match read_con.read_exact(&mut tmp_buf).await {
            Ok(_) => {}
            Err(e) => {
                error!("Draining: {}", e);
            }
        };
    }

    /// This listens to the Client-Connection and forwards the messages to the
    /// correct User-Connections
    ///
    /// Params:
    /// * id: The ID of the Client
    /// * read_con: The Reader-Half of the Client-Connection
    /// * user_cons: The User-Connections
    pub async fn receiver(
        id: u32,
        mut read_con: tokio::net::tcp::OwnedReadHalf,
        user_cons: std::sync::Arc<Connections<spsc::StreamWriter<Message>>>,
    ) {
        loop {
            let mut head_buf = [0; 13];
            let header = match read_con.read_exact(&mut head_buf).await {
                Ok(_) => {
                    let h = MessageHeader::deserialize(head_buf);
                    if h.is_none() {
                        error!("[{}] Deserializing Header: {:?}", id, head_buf);
                        continue;
                    }
                    h.unwrap()
                }
                Err(e) => {
                    error!("[{}] Reading from Client-Connection: {}", id, e);
                    return;
                }
            };

            if let MessageType::Heartbeat = header.get_kind() {
                debug!("[{}] Received Heartbeat", id);
                continue;
            }

            match header.get_kind() {
                MessageType::Data => {}
                _ => {
                    error!(
                        "[{}][{}] Unexpected Operation: {:?}",
                        id,
                        header.get_id(),
                        header.get_kind()
                    );
                    Client::drain(&mut read_con, header.get_length() as usize).await;
                }
            };

            // Forwarding the message to the actual user
            let stream = match user_cons.get(header.get_id()) {
                Some(s) => s,
                None => {
                    error!("[{}] No Connection found with ID: {}", id, header.get_id());
                    // TODO
                    // This also then needs to drain the next data that belongs to
                    // this incorrect request as this otherwise it will bring
                    // everyting else out of order as well
                    Client::drain(&mut read_con, header.get_length() as usize).await;
                    continue;
                }
            };

            let body_length = header.get_length() as usize;
            let mut body_buf = vec![0; body_length];
            match read_con.read_exact(&mut body_buf).await {
                Ok(_) => {}
                Err(e) => {
                    error!("[{}][{}] Reading from Client: {}", id, header.get_id(), e);
                }
            };

            let user_id = header.get_id();
            match stream.send(Message::new(header, body_buf)) {
                Ok(_) => {}
                Err(e) => {
                    error!("[{}][{}] Adding to User-Queue: {}", id, user_id, e);
                }
            };
        }
    }

    /// This Receives messages from users and then forwards them to the
    /// Client-Connection
    ///
    /// Params:
    /// * id: The ID of the Client
    /// * write_con: The Write-Half of the Client-Connection
    /// * queue: The Queue of messages to forward to the Client
    pub async fn sender(
        id: u32,
        mut write_con: tokio::net::tcp::OwnedWriteHalf,
        mut queue: tokio::sync::mpsc::UnboundedReceiver<Message>,
    ) {
        loop {
            let msg = match queue.recv().await {
                Some(m) => m,
                None => {
                    error!("[{}][Sender] Receiving Message from Queue", id);
                    return;
                }
            };

            let data = msg.serialize();
            let total_data_length = data.len();
            match write_con.write_all(&data).await {
                Ok(_) => {
                    debug!("[{}][Sender] Send {} out bytes", id, total_data_length);
                }
                Err(e) => {
                    error!("[{}][Sender] Sending Message: {}", id, e);
                    return;
                }
            };
        }
    }
}
