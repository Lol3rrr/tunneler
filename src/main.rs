use std::io::prelude::*;
use structopt::StructOpt;
use tunneler::*;

use log::info;

fn default_key_path() -> String {
    let mut key_path = dirs::home_dir().unwrap();
    key_path.push(".tunneler");
    key_path.push("key");
    key_path.as_path().to_str().unwrap().to_string()
}

fn default_threads() -> usize {
    let core_count = num_cpus::get();

    let min_threads = 3;
    std::cmp::max(min_threads, core_count)
}

fn main() {
    env_logger::init();

    match Arguments::from_args() {
        Arguments::Client {
            external_port,
            listen_port,
            server_ip,
            target_port,
            target_ip,
            key_path,
            threads,
        } => {
            let key_path = match key_path {
                Some(k) => k,
                None => default_key_path(),
            };
            let threads = match threads {
                Some(t) => t,
                None => default_threads(),
            };

            info!("Threads: {}", threads);

            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(threads)
                .enable_io()
                .enable_time()
                .build()
                .unwrap();

            let client = CliClient::new_from_args(
                server_ip,
                listen_port,
                external_port,
                key_path,
                target_ip,
                target_port,
            );
            rt.block_on(client.start()).unwrap();
        }
        Arguments::Server {
            external_port,
            listen_port,
            key_path,
            threads,
        } => {
            let key_path = match key_path {
                Some(k) => k,
                None => default_key_path(),
            };
            let threads = match threads {
                Some(t) => t,
                None => default_threads(),
            };

            info!("Threads: {}", threads);

            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(threads)
                .enable_io()
                .enable_time()
                .build()
                .unwrap();

            let server = CliServer::new_from_args(external_port, listen_port, key_path);
            rt.block_on(server.start()).unwrap();
        }
        Arguments::KeyGen { key_path } => {
            let key_path = match key_path {
                Some(k) => k,
                None => default_key_path(),
            };

            info!("Generating Server-Key");
            let raw_key = general::generate_key(64);
            let key = base64::encode(raw_key);

            let path = std::path::Path::new(&key_path);

            std::fs::create_dir_all(path.parent().unwrap())
                .expect("Could not create directory for key-file");
            let mut key_file = std::fs::File::create(&path).expect("Could not create key-file");
            key_file
                .write_all(key.as_bytes())
                .expect("Could not write to key-file");
            info!("Wrote Key to file: {}", key_path);
        }
    }
}
