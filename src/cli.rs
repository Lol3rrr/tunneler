use structopt::StructOpt;

#[derive(StructOpt)]
pub struct Arguments {
    pub command: String,

    #[structopt(short = "p")]
    pub public_port: Option<u32>,

    #[structopt(short = "l")]
    pub listen_port: Option<u32>,

    #[structopt(long = "ip")]
    pub server_ip: Option<String>,

    #[structopt(long = "out-ip", default_value = "localhost")]
    pub out_ip: String,

    #[structopt(long = "key", short = "k")]
    pub key_path: Option<String>,
}
