use crate::server::client::{Client, ClientManager};
use crate::Arguments;
use crate::{Connection, Error, Message, MessageHeader, MessageType};

use rand::rngs::OsRng;
use rand::Rng;
use rsa::{PaddingScheme, PublicKeyParts, RSAPrivateKey, RSAPublicKey};
use tokio::net::TcpListener;

use log::{error, info};

#[derive(Debug)]
pub struct Server {
    listen_port: u32,
    public_port: u32,
    key: Vec<u8>,
}

impl Server {
    pub fn new_from_args(cli: Arguments) -> Result<Server, Error> {
        let raw_key = std::fs::read(cli.key_path.unwrap()).expect("Reading Key file");
        let key = base64::decode(raw_key).unwrap();

        Ok(Server {
            listen_port: cli.listen_port.expect("Loading Listen-Port"),
            public_port: cli.public_port.expect("Loading Public-Port"),
            key,
        })
    }

    // The validation flow is like this
    //
    // 1. Client connects
    // 2. Server generates and sends public key
    // 3. Client sends encrypted password/key
    // 4. Server decrypts the message and checks if the password/key is valid
    // 5a. If valid: Server sends an Acknowledge message and its done
    // 5b. If invalid: Server closes the connection
    async fn validate_connection(con: std::sync::Arc<Connection>, key: &[u8]) -> bool {
        // Step 2
        let mut rng = OsRng;
        let priv_key = RSAPrivateKey::new(&mut rng, 2048).expect("Failed to generate private key");
        let pub_key = RSAPublicKey::from(&priv_key);

        let pub_n_bytes = pub_key.n().to_bytes_le();
        let mut pub_e_bytes = pub_key.e().to_bytes_le();

        let mut data = pub_n_bytes;
        data.append(&mut pub_e_bytes);

        let msg_header = MessageHeader::new(0, MessageType::Key, data.len() as u64);
        let msg = Message::new(msg_header, data);

        match con.write(&msg.serialize()).await {
            Ok(_) => {}
            Err(e) => {
                println!("Error sending Key: {}", e);
                return false;
            }
        };

        // Step 4
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
        if *header.get_kind() != MessageType::Verify {
            return false;
        }

        let mut key_buf = [0; 4092];
        let recv_encrypted_key = match con.read(&mut key_buf).await {
            Ok(0) => {
                return false;
            }
            Ok(n) => &key_buf[0..n],
            Err(e) => {
                println!("Could not read key: {}", e);
                return false;
            }
        };

        let recv_key = match priv_key.decrypt(PaddingScheme::PKCS1v15Encrypt, recv_encrypted_key) {
            Ok(raw_key) => raw_key,
            Err(e) => {
                println!("Error decrypting received-key: {}", e);
                return false;
            }
        };

        // Step 5
        if recv_key != key {
            // Step 5a
            println!("The keys are not matching");
            return false;
        }

        // Step 5b
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
        key: Vec<u8>,
        clients: std::sync::Arc<ClientManager>,
    ) {
        loop {
            // Get Client
            let client = match listen.accept().await {
                Ok((socket, _)) => std::sync::Arc::new(Connection::new(socket)),
                Err(e) => {
                    error!("Accepting client-connection: {}", e);
                    continue;
                }
            };

            if !Server::validate_connection(client.clone(), &key).await {
                error!("Rejected Client");
                continue;
            }

            info!("Accepted client");

            let c_id: u32 = rand::thread_rng().gen();
            let (queue_tx, queue_rx) = tokio::sync::mpsc::unbounded_channel();

            let client_con = Client::new(c_id, client, clients.clone(), queue_tx);
            tokio::task::spawn(Client::sender(client_con.clone(), queue_rx));
            tokio::task::spawn(client_con.clone().read_respond());

            clients.add(client_con);
        }
    }

    pub async fn start(self) -> Result<(), Error> {
        println!("Starting...");

        let listen_bind_addr = format!("0.0.0.0:{}", self.listen_port);
        let listen_listener = TcpListener::bind(&listen_bind_addr).await?;

        let req_bind_addr = format!("0.0.0.0:{}", self.public_port);
        let req_listener = TcpListener::bind(&req_bind_addr).await?;

        let clients = std::sync::Arc::new(ClientManager::new());

        println!("Listening for clients on: {}", listen_bind_addr);
        println!("Listening for requests on: {}", req_bind_addr);

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
