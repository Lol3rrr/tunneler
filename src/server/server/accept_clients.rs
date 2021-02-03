use crate::server::client::{Client, ClientManager};
use crate::Connection;

use rand::Rng;
use tokio::net::TcpListener;

use log::{error, info};

mod validate_connection;

pub async fn accept_clients(
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

        if !validate_connection::validate_connection(client.clone(), &key).await {
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
