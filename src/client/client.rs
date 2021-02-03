use crate::Arguments;
use crate::{Connection, Connections, Destination};
use crate::{Error, Message, MessageHeader, MessageType};

use rand::RngCore;

mod close_user_connection;
mod establish_connection;
mod forward;
mod heartbeat;
mod respond;

use log::{error, info};

pub struct Client {
    server_destination: Destination,
    out_destination: Destination,
    key: Vec<u8>,
}

impl Client {
    pub fn new_from_args(cli: Arguments) -> Result<Client, Error> {
        if cli.server_ip.is_none() {
            return Err(Error::MissingConfig("IP".to_owned()));
        }

        let raw_key = std::fs::read(cli.key_path.unwrap()).expect("Reading Key File");
        let key = base64::decode(raw_key).unwrap();

        let server_dest = Destination::new(
            cli.server_ip.expect("Loading Server-Address"),
            cli.listen_port.expect("Loading Server-Listening Port"),
        );
        let out_dest = Destination::new(cli.out_ip, cli.public_port.expect("Loading Public Port"));

        Ok(Client {
            server_destination: server_dest,
            out_destination: out_dest,
            key,
        })
    }

    pub async fn heartbeat(
        send_queue: tokio::sync::mpsc::UnboundedSender<Message>,
        wait_time: std::time::Duration,
    ) {
        heartbeat::heartbeat(send_queue, wait_time).await;
    }

    /// Sends all the messages to the server
    async fn sender(
        server_con: std::sync::Arc<Connection>,
        mut queue: tokio::sync::mpsc::UnboundedReceiver<Message>,
    ) {
        loop {
            let msg = match queue.recv().await {
                Some(m) => m,
                None => {
                    info!("[Sender] All Queue-Senders have been closed");
                    return;
                }
            };

            let data = msg.serialize();
            let total_data_length = data.len();

            match server_con.write_total(&data, total_data_length).await {
                Ok(_) => {}
                Err(e) => {
                    error!("Sending Message: {}", e);
                    return;
                }
            };
        }
    }

    /// Receives all the messages from the server
    ///
    /// Then adds the message to the matching connection queue.
    /// If there is no matching queue, it creates and starts a new client,
    /// which will then be placed into the Connection Manager for further
    /// requests
    async fn receiver(
        server_con: std::sync::Arc<Connection>,
        send_queue: tokio::sync::mpsc::UnboundedSender<Message>,
        client_cons: std::sync::Arc<Connections<tokio::sync::broadcast::Sender<Message>>>,
        out_dest: &Destination,
    ) {
        loop {
            let mut head_buf = [0; 13];
            let header = match server_con.read_total(&mut head_buf, 13).await {
                Ok(_) => match MessageHeader::deserialize(head_buf) {
                    Some(s) => s,
                    None => {
                        error!("[Receiver] Deserializing Header: {:?}", head_buf);
                        return;
                    }
                },
                Err(e) => {
                    error!("[Receiver] Reading Data: {}", e);
                    return;
                }
            };

            let id = header.get_id();
            let kind = header.get_kind();
            match kind {
                MessageType::Close => {
                    info!("[Receiver] Received Close Message: {}", id);
                    close_user_connection::close_user_connection(id, &client_cons);
                    continue;
                }
                MessageType::Data => {}
                _ => {
                    error!("[Receiver][{}] Unexpected Operation: {:?}", id, kind);
                    continue;
                }
            };

            let data_length = header.get_length() as usize;
            let mut buf = vec![0; data_length];
            let msg = match server_con.read_total(&mut buf, data_length).await {
                Ok(_) => Message::new(header, buf),
                Err(e) => {
                    error!("[Receiver][{}] Receiving Data: {}", e, id);
                    close_user_connection::close_user_connection(id, &client_cons);
                    continue;
                }
            };

            let con_queue = match client_cons.get(id) {
                Some(send_queue) => send_queue.clone(),
                // In case there is no matching user-connection, create a new one
                None => {
                    // Connects out to the server
                    let raw_con = out_dest.connect().await.unwrap();
                    let (read_con, write_con) = tokio::io::split(raw_con);

                    // Setup the send channel for requests for this user
                    let (tx, rx) = tokio::sync::broadcast::channel(25);
                    // Add the Connection to the current map of user-connection
                    client_cons.set(id, tx.clone());
                    // Starting the receive and send tasks for this connection
                    tokio::task::spawn(respond::respond(
                        id,
                        send_queue.clone(),
                        read_con,
                        client_cons.clone(),
                    ));
                    tokio::task::spawn(forward::forward(write_con, rx));
                    tx
                }
            };

            match con_queue.send(msg) {
                Ok(_) => {}
                Err(e) => {
                    error!("[Receiver][{}] Adding to Queue: {}", id, e);
                }
            };
        }
    }

    pub async fn start(&self) -> Result<(), Error> {
        info!("Starting...");

        let mut attempts = 0;
        let wait_base: u64 = 2;

        loop {
            info!(
                "Conneting to server: {}",
                self.server_destination.get_full_address()
            );

            let connection_arc = match establish_connection::establish_connection(
                &self.server_destination.get_full_address(),
                &self.key,
            )
            .await
            {
                Some(c) => c,
                None => {
                    attempts += 1;
                    let raw_time = std::time::Duration::from_secs(wait_base.pow(attempts));
                    let final_wait_time = raw_time
                        .checked_add(std::time::Duration::from_millis(
                            rand::rngs::ThreadRng::default().next_u64() % 1000,
                        ))
                        .unwrap();
                    info!(
                        "Waiting {:?} before trying to connect again",
                        final_wait_time
                    );
                    tokio::time::sleep(final_wait_time).await;

                    continue;
                }
            };

            info!("Connected to server");

            attempts = 0;

            let (queue_tx, queue_rx) = tokio::sync::mpsc::unbounded_channel();
            let outgoing =
                std::sync::Arc::new(Connections::<tokio::sync::broadcast::Sender<Message>>::new());

            tokio::task::spawn(Client::sender(connection_arc.clone(), queue_rx));

            tokio::task::spawn(Client::heartbeat(
                queue_tx.clone(),
                std::time::Duration::from_secs(15),
            ));

            Client::receiver(
                connection_arc,
                queue_tx.clone(),
                outgoing,
                &self.out_destination,
            )
            .await;
        }
    }
}
