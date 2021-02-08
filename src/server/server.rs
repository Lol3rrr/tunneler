use crate::Arguments;

use tunneler_core::server::Server;

#[derive(Debug)]
pub struct CliServer {
    server: Server,
}

impl CliServer {
    pub fn new_from_args(cli: Arguments) -> Self {
        let raw_key = std::fs::read(cli.key_path.unwrap()).expect("Reading Key file");
        let key = base64::decode(raw_key).unwrap();

        let public_port = cli.public_port.expect("Loading Public-Port");
        let listen_port = cli.listen_port.expect("Loading Listen-Port");

        let server = Server::new(public_port, listen_port, key);

        Self { server }
    }

    pub async fn start(self) -> std::io::Result<()> {
        self.server.start().await
    }
}
