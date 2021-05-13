use tunneler_core::{
    metrics::Empty,
    server::{Server, Strategy},
};

#[derive(Debug)]
pub struct CliServer {
    server: Server<Empty>,
}

impl CliServer {
    pub fn new_from_args(public_strat: Strategy, listen_port: u32, key_path: String) -> Self {
        let raw_key = std::fs::read(key_path).expect("Reading Key file");
        let key = base64::decode(raw_key).unwrap();

        let server = Server::new(listen_port, public_strat, key);

        Self { server }
    }

    pub async fn start(self) -> std::io::Result<()> {
        self.server.start().await
    }
}
