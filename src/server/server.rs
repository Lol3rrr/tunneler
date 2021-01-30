use crate::Arguments;
use crate::{Connection, Connections, Error, Message, MessageHeader, MessageType};

use rand::Rng;
use tokio::net::TcpListener;

#[derive(Debug)]
pub struct Server {
    listen_port: u32,
    public_port: u32,
    key: String,
}

impl Server {
    pub fn new_from_args(cli: Arguments) -> Result<Server, Error> {
        Ok(Server {
            listen_port: cli.listen_port,
            public_port: cli.public_port,
            key: cli.key,
        })
    }

    async fn read_responses(
        client: std::sync::Arc<Connection>,
        cons: std::sync::Arc<Connections<Connection>>,
    ) {
        loop {
            let mut head_buf = [0; 13];
            let header = match client.read(&mut head_buf).await {
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
            match client.read(&mut buf).await {
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

                    match cons.get(header.get_id()) {
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

    async fn close_user_connection(
        id: u32,
        connections: std::sync::Arc<Connections<Connection>>,
        client_connection: std::sync::Arc<Connection>,
    ) {
        connections.remove(id);

        let header = MessageHeader::new(id, MessageType::Close, 0);
        let close_msg = Message::new(header, vec![0]);
        match client_connection.write(&close_msg.serialize()).await {
            Ok(_) => {}
            Err(e) => {
                println!("Sending Close Message: {}", e);
            }
        };
    }

    async fn process_connection(
        id: u32,
        socket: std::sync::Arc<Connection>,
        client: std::sync::Arc<Connection>,
        connections: std::sync::Arc<Connections<Connection>>,
    ) {
        // Reads and forwards all the data from the socket to the client
        loop {
            let mut buf = vec![0; 4092];
            // Try to read data, this may still fail with `WouldBlock`
            // if the readiness event is a false positive.
            match socket.read(&mut buf).await {
                Ok(0) => {
                    break;
                }
                Ok(n) => {
                    let header = MessageHeader::new(id, MessageType::Data, n as u64);
                    let msg = Message::new(header, buf);
                    match client.write(&msg.serialize()).await {
                        Ok(_) => {}
                        Err(e) => {
                            println!("Sending: {}", e);
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
                        tokio::task::spawn(Server::close_user_connection(id, connections, client));
                    }
                    return;
                }
            }
        }
    }

    async fn validate_connection(con: std::sync::Arc<Connection>, key: &str) -> bool {
        let mut head_buf = [0; 13];
        let header = match con.read(&mut head_buf).await {
            Ok(0) => {
                return false;
            }
            Ok(_) => {
                let msg = MessageHeader::deserialize(head_buf);
                if msg.is_none() {
                    return false;
                }
                msg.unwrap()
            }
            Err(e) => {
                println!("Error reading validate: {}", e);
                return false;
            }
        };

        if *header.get_kind() != MessageType::Establish {
            return false;
        }

        let mut key_buf = [0; 4092];
        let recv_key = match con.read(&mut key_buf).await {
            Ok(0) => {
                return false;
            }
            Ok(n) => {
                let raw_key = &key_buf[0..n];
                String::from_utf8_lossy(raw_key)
            }
            Err(e) => {
                println!("Could not read key: {}", e);
                return false;
            }
        };

        if recv_key != key {
            println!("The keys are not matching");
            return false;
        }

        let ack_header = MessageHeader::new(0, MessageType::Acknowledge, 0);
        match con.write(&ack_header.serialize()).await {
            Ok(_) => {}
            Err(e) => {
                println!("Error sending acknowledge: {}", e);
                return false;
            }
        };

        true
    }

    pub async fn start(self) -> Result<(), Error> {
        println!("Starting...");
        println!("{:?}", self);

        let listen_bind_addr = format!("127.0.0.1:{}", self.listen_port);
        let listen_listener = TcpListener::bind(&listen_bind_addr).await?;

        let req_bind_addr = format!("127.0.0.1:{}", self.public_port);
        let req_listener = TcpListener::bind(&req_bind_addr).await?;

        loop {
            // Get Client
            let client = match listen_listener.accept().await {
                Ok((socket, _)) => std::sync::Arc::new(Connection::new(socket)),
                Err(e) => {
                    println!("Accepting client-connection: {}", e);
                    continue;
                }
            };

            if !Server::validate_connection(client.clone(), &self.key).await {
                println!("Rejected Client");
                continue;
            }

            println!("Accepted client");

            let connections: std::sync::Arc<Connections<Connection>> =
                std::sync::Arc::new(Connections::new());

            tokio::task::spawn(Server::read_responses(client.clone(), connections.clone()));

            let mut rng = rand::thread_rng();

            loop {
                match req_listener.accept().await {
                    Ok((raw_socket, _)) => {
                        let id = rng.gen();

                        let socket = std::sync::Arc::new(Connection::new(raw_socket));
                        connections.set(id, socket.clone());

                        tokio::task::spawn(Server::process_connection(
                            id,
                            socket,
                            client.clone(),
                            connections.clone(),
                        ));
                    }
                    Err(e) => {
                        println!("Accepting Req-Connection: {}", e);
                        continue;
                    }
                }
            }
        }
    }
}
