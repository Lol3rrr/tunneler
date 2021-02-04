use crate::{Connections, Message};

use log::{debug, error};

pub fn close_user_connection(
    id: u32,
    users: &Connections<tokio::sync::broadcast::Sender<Message>>,
) {
    match users.remove(id) {
        Some(_) => {
            debug!("[{}] Closed connection", id);
        }
        None => {
            error!("[{}] Connection to close not found", id);
        }
    };
}
