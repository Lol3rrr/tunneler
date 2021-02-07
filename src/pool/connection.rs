use log::error;

pub type ReadConnection = Connection<tokio::net::tcp::OwnedReadHalf>;
pub type WriteConnection = Connection<tokio::net::tcp::OwnedWriteHalf>;

pub enum ConnectionType {
    Read,
    Write,
}

pub struct Connection<T> {
    id: u64,
    conn: std::sync::Arc<T>,
    notify: tokio::sync::broadcast::Sender<(u64, std::sync::Arc<T>)>,
}

impl<T> Connection<T> {
    pub fn new(
        id: u64,
        conn: T,
        notify: tokio::sync::broadcast::Sender<(u64, std::sync::Arc<T>)>,
    ) -> Self {
        Self {
            id,
            conn: std::sync::Arc::new(conn),
            notify,
        }
    }
}

impl<T> AsMut<T> for Connection<T> {
    fn as_mut(&mut self) -> &mut T {
        std::sync::Arc::get_mut(&mut self.conn).unwrap()
    }
}

impl<T> Drop for Connection<T> {
    fn drop(&mut self) {
        let msg = (self.id, self.conn.clone());
        match self.notify.send(msg) {
            Ok(_) => {}
            Err(e) => {
                error!("Notifying Con-Pool of dropped Connection: {}", e);
            }
        };
    }
}

#[tokio::test]
async fn connection_drop() {
    let (tx, mut rx) = tokio::sync::broadcast::channel(2);
    let id = 1232;

    let con = Connection::new(id, 10, tx);
    // should now send a notification using the channel
    drop(con);

    let msg = rx.recv().await;
    assert_eq!(Ok((id, std::sync::Arc::new(10))), msg);
}

#[tokio::test]
async fn connection_as_mut() {
    let (tx, _rx) = tokio::sync::broadcast::channel(2);
    let id = 1232;

    let mut con = Connection::new(id, 10, tx);
    *(con.as_mut()) += 1;

    assert_eq!(11, *con.conn);
}
