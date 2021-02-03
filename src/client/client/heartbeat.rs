use crate::{Message, MessageHeader, MessageType};

use log::{debug, error};

pub async fn heartbeat(
    send_queue: tokio::sync::mpsc::UnboundedSender<Message>,
    wait_time: std::time::Duration,
) {
    loop {
        debug!("Sending Heartbeat");

        let msg_header = MessageHeader::new(0, MessageType::Heartbeat, 0);
        let msg = Message::new(msg_header, Vec::new());

        match send_queue.send(msg) {
            Ok(_) => {
                debug!("Successfully send Heartbeat");
            }
            Err(e) => {
                error!("[Heartbeat] Sending: {}", e);
                return;
            }
        };

        tokio::time::sleep(wait_time).await;
    }
}
