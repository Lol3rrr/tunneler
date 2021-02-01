use crate::Arguments;
use crate::{Connection, ConnectionManager, Connections};
use crate::{Error, Message, MessageHeader, MessageType};

use rsa::{BigUint, PaddingScheme, PublicKey, PublicKeyParts, RSAPrivateKey, RSAPublicKey};
use tokio::net::TcpStream;

pub struct Client {
    listen_port: u32,
    ip: String,
    out_ip: String,
    out_port: u32,
    key: String,
}

impl Client {
    pub fn new_from_args(cli: Arguments) -> Result<Client, Error> {
        if cli.ip.is_none() {
            return Err(Error::MissingConfig("IP".to_owned()));
        }

        let out = "localhost";

        Ok(Client {
            listen_port: cli.listen_port,
            ip: cli.ip.unwrap(),
            out_ip: out.to_owned(),
            out_port: cli.public_port,
            key: cli.key,
        })
    }

    async fn respond(
        id: u32,
        server_con: std::sync::Arc<Connection>,
        proxied_con: std::sync::Arc<mobc::Connection<ConnectionManager>>,
        users: std::sync::Arc<Connections<mobc::Connection<ConnectionManager>>>,
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
                    match server_con.write(&msg.serialize()).await {
                        Ok(_) => {}
                        Err(e) => {
                            println!("[{}][Server] Error relaying: {}", id, e);
                        }
                    };
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    println!("[{}][Proxied] Reading from proxied: {}", id, e);
                    return;
                }
            };
        }
    }

    fn close_user_connection(id: u32, users: &Connections<mobc::Connection<ConnectionManager>>) {
        users.get(id).unwrap().close();
        if users.remove(id) {
            println!("[{}] Closed connection", id);
        } else {
            println!("[{}] Connection to close not found", id);
        }
    }

    async fn read_forward(
        server_con: std::sync::Arc<Connection>,
        outgoing: std::sync::Arc<Connections<mobc::Connection<ConnectionManager>>>,
        con_pool: mobc::Pool<ConnectionManager>,
        msg: Message,
    ) {
        let header = msg.get_header();
        let id = header.get_id();
        let out_con = match outgoing.get(id) {
            None => {
                let result = con_pool.get().await.unwrap();
                let result_arc = std::sync::Arc::new(result);
                outgoing.set(id, result_arc.clone());
                tokio::task::spawn(Client::respond(
                    id,
                    server_con,
                    result_arc.clone(),
                    outgoing.clone(),
                ));
                result_arc
            }
            Some(s) => s.clone(),
        };

        match out_con.write(msg.get_data()).await {
            Ok(_) => {}
            Err(e) => {
                println!("[Proxied] {}", e);
            }
        };
    }

    async fn handle_connection(
        server_con: std::sync::Arc<Connection>,
        outgoing: std::sync::Arc<Connections<mobc::Connection<ConnectionManager>>>,
        con_pool: mobc::Pool<ConnectionManager>,
    ) -> Result<(), Error> {
        loop {
            let mut head_buf = [0; 13];
            let header = match server_con.read(&mut head_buf).await {
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
            // Try to read data, this may still fail with `WouldBlock`
            // if the readiness event is a false positive.
            match server_con.read(&mut buf).await {
                Ok(0) => continue,
                Ok(n) => {
                    if n != data_length {
                        println!(
                            "Read bytes doesnt match body length: {} != {}",
                            n, data_length
                        );
                    }

                    match header.get_kind() {
                        MessageType::Close => {
                            println!("[Server] Closed connection");
                            Client::close_user_connection(header.get_id(), &outgoing);
                        }
                        MessageType::Data => {
                            let msg = Message::new(header, buf);
                            tokio::task::spawn(Client::read_forward(
                                server_con.clone(),
                                outgoing.clone(),
                                con_pool.clone(),
                                msg,
                            ));
                        }
                        _ => {}
                    };
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    println!("[Error][Server] Reading from Req: {}", e);
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
    async fn establish_connection(adr: &str, key: &str) -> Option<std::sync::Arc<Connection>> {
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
            .encrypt(
                &mut rand::rngs::OsRng,
                PaddingScheme::PKCS1v15Encrypt,
                key.as_bytes(),
            )
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

    pub async fn start(&self) -> Result<(), Error> {
        println!("Starting...");

        let bind_ip = format!("{}:{}", self.ip, self.listen_port);
        println!("Connecting to: {}", bind_ip);

        let manager = ConnectionManager::new(&self.out_ip, self.out_port);
        let pool = mobc::Pool::builder()
            .max_open(25)
            .max_idle(10)
            .build(manager);

        let outgoing: std::sync::Arc<Connections<mobc::Connection<ConnectionManager>>> =
            std::sync::Arc::new(Connections::new());

        loop {
            let connection_arc = match Client::establish_connection(&bind_ip, &self.key).await {
                Some(c) => c,
                None => {
                    continue;
                }
            };

            if let Err(e) =
                Client::handle_connection(connection_arc, outgoing.clone(), pool.clone()).await
            {
                println!("{}", e);
            }
        }
    }
}
