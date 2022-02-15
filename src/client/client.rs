use std::sync::Arc;

use tunneler_core::client::UserCon;
use tunneler_core::metrics::Empty;
use tunneler_core::Destination;
use tunneler_core::{
    client::{Client, Handler},
    Details,
};

use async_trait::async_trait;

mod forward;
mod respond;

pub struct CliClient {
    client: Client<Empty>,
    out_destination: Destination,
}

impl CliClient {
    pub fn new_from_args(
        server_ip: String,
        server_listen_port: u32,
        external_port: u16,
        key_path: String,
        target_ip: String,
        target_port: u32,
    ) -> Self {
        let raw_key = std::fs::read(key_path).expect("Reading Key File");
        let key = base64::decode(raw_key).unwrap();

        let server_dest = Destination::new(server_ip, server_listen_port);
        let out_dest = Destination::new(target_ip, target_port);

        let client = Client::new(server_dest, external_port, key);

        Self {
            client,
            out_destination: out_dest,
        }
    }

    pub async fn start(self) -> std::io::Result<()> {
        let handler = ForwardHandler {
            destination: self.out_destination,
        };

        self.client.start(Arc::new(handler)).await
    }
}

pub struct ForwardHandler {
    destination: Destination,
}

#[async_trait]
impl Handler for ForwardHandler {
    async fn new_con(self: Arc<Self>, id: u32, _: Details, connection: UserCon) {
        let con = match self.destination.connect().await {
            Ok(c) => c,
            Err(e) => {
                log::error!("Connecting to Destination: {:?}", e);
                return;
            }
        };
        let (read_con, write_con) = con.into_split();

        let (rx, tx) = connection.into_split();

        // Start the new task
        tokio::task::spawn(respond::respond(id, tx, read_con));

        // This can stay on the current task, because that is already
        // running a seperate tokio::Task and will therefore not
        // hold up anything else
        forward::forward(write_con, rx).await;
    }
}
