use crate::{Connection, Connections};

use log::error;

pub fn close_user_connection(id: u32, users: &Connections<Connection>) {
    match users.remove(id) {
        Some(s) => {
            s.1.close();
            error!("[{}] Closed connection", id);
        }
        None => {
            error!("[{}] Connection to close not found", id);
        }
    };
}
