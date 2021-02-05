use crate::server::client::{Client, ClientManager};

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
        let mut client_socket = match listen.accept().await {
            Ok((socket, _)) => socket,
            Err(e) => {
                error!("Accepting client-connection: {}", e);
                continue;
            }
        };

        if !validate_connection::validate_connection(&mut client_socket, &key).await {
            error!("Rejected Client");
            continue;
        }

        let c_id: u32 = rand::thread_rng().gen();

        info!("Accepted client: {}", c_id);

        let (rx, tx) = client_socket.into_split();

        let (queue_tx, queue_rx) = tokio::sync::mpsc::unbounded_channel();

        let client = Client::new(c_id, clients.clone(), queue_tx);

        tokio::task::spawn(Client::sender(c_id, tx, queue_rx, clients.clone()));
        tokio::task::spawn(Client::receiver(
            c_id,
            rx,
            client.get_user_cons(),
            clients.clone(),
        ));

        let n_client_count = clients.add(client);
        info!("Connected Clients: {}", n_client_count);
    }
}
