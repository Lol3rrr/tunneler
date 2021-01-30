use structopt::StructOpt;

#[derive(StructOpt)]
pub struct Arguments {
    pub command: String,

    #[structopt(short = "p")]
    pub public_port: u32,

    #[structopt(short = "l")]
    pub listen_port: u32,

    #[structopt(long)]
    pub ip: Option<String>,

    #[structopt(long, short = "k")]
    pub key: String,
}
