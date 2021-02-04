use crate::server::client::ClientManager;
use crate::Arguments;
use crate::{Connection, Error};

use rand::Rng;
use tokio::net::TcpListener;

use log::{error, info};

mod accept_clients;

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

    pub async fn start(self) -> Result<(), Error> {
        info!("Starting...");

        let listen_bind_addr = format!("0.0.0.0:{}", self.listen_port);
        let listen_listener = TcpListener::bind(&listen_bind_addr).await?;

        let req_bind_addr = format!("0.0.0.0:{}", self.public_port);
        let req_listener = TcpListener::bind(&req_bind_addr).await?;

        let clients = std::sync::Arc::new(ClientManager::new());

        info!("Listening for clients on: {}", listen_bind_addr);
        info!("Listening for requests on: {}", req_bind_addr);

        // Task to async accept new clients
        tokio::task::spawn(accept_clients::accept_clients(
            listen_listener,
            self.key,
            clients.clone(),
        ));

        let mut rng = rand::thread_rng();

        loop {
            let socket = match req_listener.accept().await {
                Ok((raw_socket, _)) => Connection::new(raw_socket),
                Err(e) => {
                    error!("Accepting Req-Connection: {}", e);
                    continue;
                }
            };

            let id = rng.gen();
            let client = clients.get();
            if client.is_none() {
                error!("Could not obtain a Client-Connection");
                continue;
            }

            client.unwrap().new_con(id, socket);
        }
    }
}
