use std::io::prelude::*;
use structopt::StructOpt;
use tunneler::*;

#[derive(Debug)]
enum Command {
    Server,
    Client,
    GenerateKey,
}

/// This command turns the raw string command into the Enum or returns None
/// in case the input was not valid
fn parse_command(cmd: &str) -> Option<Command> {
    match cmd {
        "server" => Some(Command::Server),
        "client" => Some(Command::Client),
        "key-gen" => Some(Command::GenerateKey),
        _ => None,
    }
}

fn main() {
    let mut arguments = Arguments::from_args();

    if arguments.key_path.is_none() {
        let mut key_path = dirs::home_dir().unwrap();
        key_path.push(".tunneler");
        key_path.push("key");
        arguments.key_path = Some(key_path.as_path().to_str().unwrap().to_string());
    }

    let command = parse_command(&arguments.command);
    if command.is_none() {
        println!("Invalid command: '{}'", arguments.command);
        std::process::exit(-1);
    }

    let core_count = num_cpus::get();
    println!("Cores: {}", core_count);

    let threads = std::cmp::max(2, core_count);
    println!("Threads: {}", threads);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(threads)
        .enable_io()
        .enable_time()
        .build()
        .unwrap();

    match command.unwrap() {
        Command::Server => {
            let server = Server::new_from_args(arguments).unwrap();
            rt.block_on(server.start()).unwrap();
        }
        Command::Client => {
            rt.block_on(Client::new_from_args(arguments).unwrap().start())
                .unwrap();
        }
        Command::GenerateKey => {
            println!("Generating Server-Key");
            let raw_key = general::generate_key(64);
            let key = base64::encode(raw_key);

            let raw_path = arguments.key_path.unwrap();
            let path = std::path::Path::new(&raw_path);

            std::fs::create_dir_all(path.parent().unwrap())
                .expect("Could not create directory for key-file");
            let mut key_file = std::fs::File::create(&path).expect("Could not create key-file");
            key_file
                .write_all(key.as_bytes())
                .expect("Could not write to key-file");
            println!("Wrote Key to file: {}", raw_path);
        }
    };
}
