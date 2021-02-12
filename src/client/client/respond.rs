use tunneler_core::client::queues;

use log::error;
use tokio::io::AsyncReadExt;

pub async fn respond(
    id: u32,
    send_queue: queues::Sender,
    mut read_user_con: tokio::net::tcp::OwnedReadHalf,
) {
    loop {
        let mut buf = vec![0; 4092];
        match read_user_con.read(&mut buf).await {
            Ok(0) => {
                break;
            }
            Ok(n) => {
                match send_queue.send(buf, n as u64).await {
                    true => {}
                    false => {
                        error!("[{}] Adding Data to Queue", id);
                    }
                };
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                error!("[{}] Reading: {}", id, e);
                break;
            }
        };
    }
}
