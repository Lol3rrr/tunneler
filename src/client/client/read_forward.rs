use crate::{Connection, Connections, Destination, Message};

mod respond;

pub async fn read_forward(
    send_queue: tokio::sync::mpsc::UnboundedSender<Message>,
    outgoing: std::sync::Arc<Connections<Connection>>,
    out_dest: &Destination,
    msg: Message,
) {
    let header = msg.get_header();
    let id = header.get_id();
    let out_con = match outgoing.get(id) {
        None => {
            let result = std::sync::Arc::new(out_dest.connect().await.unwrap());
            outgoing.set(id, result.clone());
            tokio::task::spawn(respond::respond(
                id,
                send_queue,
                result.clone(),
                outgoing.clone(),
            ));
            result
        }
        Some(s) => s.clone(),
    };

    match out_con.write(msg.get_data()).await {
        Ok(_) => {}
        Err(e) => {
            println!("[Proxied] {}", e);
        }
    };
}
