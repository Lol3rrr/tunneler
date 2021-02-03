use crate::Arguments;
use crate::Pool;
use crate::{Connection, Connections};
use crate::{Error, Message, MessageHeader, MessageType};

use rand::RngCore;
use rsa::{BigUint, PaddingScheme, PublicKey, RSAPublicKey};
use tokio::net::TcpStream;

mod close_user_connection;
mod heartbeat;
mod read_forward;

use log::{debug, error, info};

pub struct Client {
    listen_port: u32,
    ip: String,
    out_ip: String,
    out_port: u32,
    key: Vec<u8>,
}

impl Client {
    pub fn new_from_args(cli: Arguments) -> Result<Client, Error> {
        if cli.server_ip.is_none() {
            return Err(Error::MissingConfig("IP".to_owned()));
        }

        let raw_key = std::fs::read(cli.key_path.unwrap()).expect("Reading Key File");
        let key = base64::decode(raw_key).unwrap();

        Ok(Client {
            listen_port: cli.listen_port.expect("Loading Listen-Port"),
            ip: cli.server_ip.unwrap(),
            out_ip: cli.out_ip,
            out_port: cli.public_port.expect("Loading Public-Port"),
            key,
        })
    }

    async fn handle_connection(
        server_con: std::sync::Arc<Connection>,
        send_queue: tokio::sync::mpsc::UnboundedSender<Message>,
        outgoing: std::sync::Arc<Connections<Connection>>,
        con_pool: std::sync::Arc<Pool>,
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
                    tokio::task::spawn(read_forward::read_forward(
                        send_queue.clone(),
                        outgoing.clone(),
                        con_pool.clone(),
                        msg,
                    ));
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

    // The validation flow is like this
    //
    // 1. Client connects
    // 2. Server generates and sends public key
    // 3. Client sends encrypted password/key
    // 4. Server decrypts the message and checks if the password/key is valid
    // 5a. If valid: Server sends an Acknowledge message and its done
    // 5b. If invalid: Server closes the connection
    async fn establish_connection(adr: &str, key: &[u8]) -> Option<std::sync::Arc<Connection>> {
        let connection = match TcpStream::connect(&adr).await {
            Ok(c) => c,
            Err(e) => {
                println!("Establishing-Connection: {}", e);
                return None;
            }
        };
        let connection_arc = std::sync::Arc::new(Connection::new(connection));

        // Step 2 - Receive
        let mut head_buf = [0; 13];
        let header = match connection_arc.read(&mut head_buf).await {
            Ok(0) => {
                return None;
            }
            Ok(_) => {
                let msg = MessageHeader::deserialize(head_buf);
                msg.as_ref()?;
                msg.unwrap()
            }
            Err(e) => {
                println!("Error reading Message-Header: {}", e);
                return None;
            }
        };
        if *header.get_kind() != MessageType::Key {
            return None;
        }

        let mut key_buf = [0; 4092];
        let mut recv_pub_key = match connection_arc.read(&mut key_buf).await {
            Ok(0) => {
                return None;
            }
            Ok(n) => key_buf[0..n].to_vec(),
            Err(e) => {
                println!("Error reading pub-key: {}", e);
                return None;
            }
        };

        let e_bytes = recv_pub_key.split_off(256);
        let n_bytes = recv_pub_key;

        let pub_key = RSAPublicKey::new(
            BigUint::from_bytes_le(&n_bytes),
            BigUint::from_bytes_le(&e_bytes),
        )
        .expect("Could not create Public-Key");

        let encrypted_key = pub_key
            .encrypt(&mut rand::rngs::OsRng, PaddingScheme::PKCS1v15Encrypt, key)
            .expect("Could not encrypt Key");

        let msg_header = MessageHeader::new(0, MessageType::Verify, encrypted_key.len() as u64);
        let msg = Message::new(msg_header, encrypted_key);

        match connection_arc.write(&msg.serialize()).await {
            Ok(_) => {}
            Err(e) => {
                println!("Validating-Key: {}", e);
                return None;
            }
        };

        loop {
            let mut buf = [0; 13];
            match connection_arc.read(&mut buf).await {
                Ok(0) => {
                    return None;
                }
                Ok(_) => {
                    let header = MessageHeader::deserialize(buf);
                    let header = header.as_ref()?;

                    if *header.get_kind() != MessageType::Acknowledge {
                        return None;
                    }

                    break;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    println!("Error while reading response: {}", e);
                    return None;
                }
            };
        }

        Some(connection_arc)
    }

    pub async fn heartbeat(
        send_queue: tokio::sync::mpsc::UnboundedSender<Message>,
        wait_time: std::time::Duration,
    ) {
        heartbeat::heartbeat(send_queue, wait_time).await;
    }

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
            let mut left_to_send = total_data_length;
            let mut offset = 0;
            while left_to_send > 0 {
                match server_con.write(&data[offset..offset + left_to_send]).await {
                    Ok(0) => {
                        error!("[Sender] Wrote 0 bytes");
                        return;
                    }
                    Ok(n) => {
                        offset += n;
                        left_to_send -= n;

                        debug!("[Sender] Send {} out bytes", n);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        continue;
                    }
                    Err(e) => {
                        error!("Sending Message: {}", e);
                        return;
                    }
                }
            }
        }
    }

    pub async fn start(&self) -> Result<(), Error> {
        info!("Starting...");

        let bind_ip = format!("{}:{}", self.ip, self.listen_port);

        let pool = std::sync::Arc::new(Pool::new(format!("{}:{}", self.out_ip, self.out_port)));

        let outgoing: std::sync::Arc<Connections<Connection>> =
            std::sync::Arc::new(Connections::new());

        let mut attempts = 0;
        let wait_base: u64 = 2;

        loop {
            info!("Conneting to server: {}", bind_ip);

            let connection_arc = match Client::establish_connection(&bind_ip, &self.key).await {
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

            tokio::task::spawn(Client::sender(connection_arc.clone(), queue_rx));

            tokio::task::spawn(Client::heartbeat(
                queue_tx.clone(),
                std::time::Duration::from_secs(15),
            ));

            if let Err(e) =
                Client::handle_connection(connection_arc, queue_tx, outgoing.clone(), pool.clone())
                    .await
            {
                error!("{}", e);
            }
        }
    }
}
