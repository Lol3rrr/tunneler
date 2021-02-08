use crate::Arguments;

use tunneler_core::client::queues;
use tunneler_core::client::Client;
use tunneler_core::message::Message;
use tunneler_core::streams::mpsc;
use tunneler_core::Destination;

mod forward;
mod respond;

pub struct CliClient {
    client: Client,
    out_destination: Destination,
}

impl CliClient {
    pub fn new_from_args(cli: Arguments) -> Self {
        if cli.server_ip.is_none() {
            panic!("Missing IP");
        }

        let raw_key = std::fs::read(cli.key_path.unwrap()).expect("Reading Key File");
        let key = base64::decode(raw_key).unwrap();

        let server_dest = Destination::new(
            cli.server_ip.expect("Loading Server-Address"),
            cli.listen_port.expect("Loading Server-Listening Port"),
        );
        let out_dest = Destination::new(cli.out_ip, cli.public_port.expect("Loading Public Port"));

        let client = Client::new(server_dest, key);

        Self {
            client,
            out_destination: out_dest,
        }
    }

    async fn handler(
        id: u32,
        rx: mpsc::StreamReader<Message>,
        tx: queues::Sender,
        dest: Option<Destination>,
    ) {
        let con = dest.unwrap().connect().await.unwrap();
        let (read_con, write_con) = con.into_split();

        // Starting the receive and send tasks for this connection
        tokio::task::spawn(respond::respond(id, tx, read_con));
        tokio::task::spawn(forward::forward(write_con, rx));
    }

    pub async fn start(self) -> std::io::Result<()> {
        self.client
            .start(Self::handler, Some(self.out_destination))
            .await
    }
}
