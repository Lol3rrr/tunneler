use structopt::StructOpt;
use tunneler::*;

#[derive(Debug)]
enum Command {
    Server,
    Client,
}

/// This command turns the raw string command into the Enum or returns None
/// in case the input was not valid
fn parse_command(cmd: &str) -> Option<Command> {
    match cmd {
        "server" => Some(Command::Server),
        "client" => Some(Command::Client),
        _ => None,
    }
}

fn main() {
    let arguments = Arguments::from_args();

    let command = parse_command(&arguments.command);
    if command.is_none() {
        println!("Invalid command: '{}'", arguments.command);
        std::process::exit(-1);
    }

    let core_count = num_cpus::get();
    println!("Cores: {}", core_count);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(core_count)
        .enable_io()
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
    };
}
