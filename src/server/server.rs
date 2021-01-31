use crate::server::client::{Client, ClientManager};
use crate::Arguments;
use crate::{Connection, Error, MessageHeader, MessageType};

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

    async fn accept_clients(
        listen: TcpListener,
        key: String,
        clients: std::sync::Arc<ClientManager>,
    ) {
        loop {
            // Get Client
            let client = match listen.accept().await {
                Ok((socket, _)) => std::sync::Arc::new(Connection::new(socket)),
                Err(e) => {
                    println!("Accepting client-connection: {}", e);
                    continue;
                }
            };

            if !Server::validate_connection(client.clone(), &key).await {
                println!("Rejected Client");
                continue;
            }

            println!("Accepted client");

            let c_id: u32 = rand::thread_rng().gen();
            let client_con = Client::new(c_id, client, clients.clone());
            tokio::task::spawn(client_con.clone().read_respond());

            clients.add(client_con);
        }
    }

    pub async fn start(self) -> Result<(), Error> {
        println!("Starting...");
        println!("{:?}", self);

        let listen_bind_addr = format!("127.0.0.1:{}", self.listen_port);
        let listen_listener = TcpListener::bind(&listen_bind_addr).await?;

        let req_bind_addr = format!("127.0.0.1:{}", self.public_port);
        let req_listener = TcpListener::bind(&req_bind_addr).await?;

        let clients = std::sync::Arc::new(ClientManager::new());

        // Task to async accept new clients
        tokio::task::spawn(Server::accept_clients(
            listen_listener,
            self.key,
            clients.clone(),
        ));

        let mut rng = rand::thread_rng();

        loop {
            let socket = match req_listener.accept().await {
                Ok((raw_socket, _)) => std::sync::Arc::new(Connection::new(raw_socket)),
                Err(e) => {
                    println!("Accepting Req-Connection: {}", e);
                    continue;
                }
            };

            let id = rng.gen();
            let client = clients.get();
            if client.is_none() {
                println!("Could not obtain a Client-Connection");
                continue;
            }

            client.unwrap().new_con(id, socket);
        }
    }
}
