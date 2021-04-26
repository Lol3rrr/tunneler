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

    async fn handler(
        id: u32,
        rx: mpsc::StreamReader<Message>,
        tx: queues::Sender,
        dest: Option<Destination>,
    ) {
        let con = dest.unwrap().connect().await.unwrap();
        let (read_con, write_con) = con.into_split();

        // Start the new task
        tokio::task::spawn(respond::respond(id, tx, read_con));

        // This can stay on the current task, because that is already
        // running a seperate tokio::Task and will therefore not
        // hold up anything else
        forward::forward(write_con, rx).await;
    }

    pub async fn start(self) -> std::io::Result<()> {
        self.client
            .start(Self::handler, Some(self.out_destination))
            .await
    }
}
