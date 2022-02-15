use log::error;
use tokio::io::AsyncReadExt;
use tunneler_core::client::{user_con::OwnedSender, Sender};

pub async fn respond(
    id: u32,
    send_queue: OwnedSender,
    mut read_user_con: tokio::net::tcp::OwnedReadHalf,
) {
    loop {
        let mut buf = vec![0; 4092];
        match read_user_con.read(&mut buf).await {
            Ok(0) => {
                break;
            }
            Ok(n) => {
                match send_queue.send_msg(buf, n as u64).await {
                    Ok(_) => {}
                    Err(e) => {
                        error!("[{}] Sending Message: {}", id, e);
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
