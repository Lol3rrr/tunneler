use crate::pool::connection::{ReadConnection, WriteConnection};
use crate::Destination;

use log::error;

use tokio::sync::broadcast::{self, Receiver, Sender};

use rand::Rng;

pub struct Manager {
    // The Destination where each connetion should connect to
    dest: Destination,
    // The maximum number of connects held by this pool
    max_cons: usize,
    // All available Connection-Pairs
    avail_cons: tokio::sync::Mutex<Vec<(ReadConnection, WriteConnection)>>,
    // The Transmit-Half for all notifcations regarding the Read-Parts
    notify_read_tx: Sender<(u64, std::sync::Arc<tokio::net::tcp::OwnedReadHalf>)>,
    // The Receive-Half for all notifcations regarding the Read-Parts
    notify_read_rx:
        tokio::sync::Mutex<Receiver<(u64, std::sync::Arc<tokio::net::tcp::OwnedReadHalf>)>>,
    // The Transmit-Half for all notifcations regarding the Write-Parts
    notify_write_tx: Sender<(u64, std::sync::Arc<tokio::net::tcp::OwnedWriteHalf>)>,
    // The Receiver-Half for all notifcations regarding the Write-Parts
    notify_write_rx:
        tokio::sync::Mutex<Receiver<(u64, std::sync::Arc<tokio::net::tcp::OwnedWriteHalf>)>>,
    recovered_reads: tokio::sync::Mutex<Vec<(u64, tokio::net::tcp::OwnedReadHalf)>>,
    recovered_writes: tokio::sync::Mutex<Vec<(u64, tokio::net::tcp::OwnedWriteHalf)>>,
}

impl Manager {
    pub fn new(dest: Destination, max_cons: usize) -> Self {
        let (read_tx, read_rx) = broadcast::channel(10);
        let (write_tx, write_rx) = broadcast::channel(10);

        Self {
            dest,
            max_cons,
            avail_cons: tokio::sync::Mutex::new(Vec::with_capacity(max_cons)),
            notify_read_tx: read_tx,
            notify_read_rx: tokio::sync::Mutex::new(read_rx),
            notify_write_tx: write_tx,
            notify_write_rx: tokio::sync::Mutex::new(write_rx),
            recovered_reads: tokio::sync::Mutex::new(Vec::new()),
            recovered_writes: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    fn find_con<C>(id: u64, arr: &[(u64, C)]) -> Option<usize> {
        for (index, (tmp_id, _)) in arr.iter().enumerate() {
            if id == *tmp_id {
                return Some(index);
            }
        }

        None
    }

    /// Takes the Mutex for the Read-Receiver and then enters
    /// an infinite loop that always tries to receive a Read
    /// Half that will then be made available again once the
    /// Write Half will also be received by another process
    async fn recover_read_loop(arc: std::sync::Arc<Self>) {
        let mut rx = arc.notify_read_rx.lock().await;
        loop {
            let (id, raw_read_con) = match rx.recv().await {
                Ok(m) => m,
                Err(e) => {
                    error!("Reading Pool-Read-Receiver: {}", e);
                    return;
                }
            };

            let read_con = std::sync::Arc::try_unwrap(raw_read_con).unwrap();

            // Checking if the Write Part was already returned
            let mut tmp_writes = arc.recovered_writes.lock().await;
            if let Some(index) = Manager::find_con(id, &tmp_writes) {
                let (_, write_con) = tmp_writes.remove(index);

                let mut avail_cons = arc.avail_cons.lock().await;

                let pool_read_con = ReadConnection::new(id, read_con, arc.notify_read_tx.clone());
                let pool_write_con =
                    WriteConnection::new(id, write_con, arc.notify_write_tx.clone());
                avail_cons.push((pool_read_con, pool_write_con));

                continue;
            };
            drop(tmp_writes);

            let mut tmp_reads = arc.recovered_reads.lock().await;
            tmp_reads.push((id, read_con));
        }
    }

    /// Takes the Mutex for the Write-Receiver and then enters
    /// an infinite loop that always tries to receive a Writer
    /// Half that will then be made available again once the
    /// Read Half will also be received by another process
    async fn recover_write_loop(arc: std::sync::Arc<Self>) {
        let mut rx = arc.notify_write_rx.lock().await;
        loop {
            let (id, raw_write_con) = match rx.recv().await {
                Ok(m) => m,
                Err(e) => {
                    error!("Reading Pool-Write-Receiver: {}", e);
                    return;
                }
            };

            let write_con = std::sync::Arc::try_unwrap(raw_write_con).unwrap();

            // Checking if the Write Part was already returned
            let mut tmp_reads = arc.recovered_reads.lock().await;
            if let Some(index) = Manager::find_con(id, &tmp_reads) {
                let (_, read_con) = tmp_reads.remove(index);

                let mut avail_cons = arc.avail_cons.lock().await;

                let pool_read_con = ReadConnection::new(id, read_con, arc.notify_read_tx.clone());
                let pool_write_con =
                    WriteConnection::new(id, write_con, arc.notify_write_tx.clone());
                avail_cons.push((pool_read_con, pool_write_con));

                continue;
            };
            drop(tmp_reads);

            let mut tmp_writes = arc.recovered_writes.lock().await;
            tmp_writes.push((id, write_con));
        }
    }

    /// Starts 2 Tasks that are used to "recover" the dropped connections
    /// as well as fills the pool with initial connections
    pub async fn start(pool_arc: std::sync::Arc<Self>) {
        let mut cons = pool_arc.avail_cons.lock().await;
        for _ in 0..pool_arc.max_connections() {
            match pool_arc.establish_new_con().await {
                Ok((read, write)) => {
                    cons.push((read, write));
                }
                Err(e) => {
                    error!("Establishing Connection: {}", e);
                    continue;
                }
            }
        }

        drop(cons);

        tokio::task::spawn(Manager::recover_read_loop(pool_arc.clone()));
        tokio::task::spawn(Manager::recover_write_loop(pool_arc));
    }

    async fn establish_new_con(&self) -> std::io::Result<(ReadConnection, WriteConnection)> {
        let raw_con = self.dest.connect().await?;
        let (r, w) = raw_con.into_split();

        let con_id = rand::thread_rng().gen();
        let read_con = ReadConnection::new(con_id, r, self.notify_read_tx.clone());
        let write_con = WriteConnection::new(con_id, w, self.notify_write_tx.clone());

        Ok((read_con, write_con))
    }

    /// Returns a valid Read/Write Connection pair from the pool.
    /// Each connection can be used seperately although they are only returned
    /// to the pool once both have been dropped.
    pub async fn get(&self) -> std::io::Result<(ReadConnection, WriteConnection)> {
        let mut cons = self.avail_cons.lock().await;
        if cons.is_empty() {
            return self.establish_new_con().await;
        }

        Ok(cons.remove(0))
    }

    pub async fn available_connections(&self) -> usize {
        let cons = self.avail_cons.lock().await;
        cons.len()
    }
    pub fn max_connections(&self) -> usize {
        self.max_cons
    }
}

#[tokio::test]
async fn new_manager() {
    let max_cons = 5;
    let manager = Manager::new(Destination::new("localhost".to_owned(), 12345), max_cons);

    assert_eq!(max_cons, manager.max_connections());
    assert_eq!(0, manager.available_connections().await);
}

#[cfg(test)]
async fn test_tcp_server_accept(listener: tokio::net::TcpListener, accept_count: u32) {
    for _x in 0..accept_count {
        match listener.accept().await {
            Ok(_) => {}
            Err(e) => {
                println!("Could not accept: {}", e);
            }
        };
    }
}

#[tokio::test]
async fn manager_start_populates_cons() {
    let port = 8080;

    let bind_adr = format!("127.0.0.1:{}", port);
    let listener_result = tokio::net::TcpListener::bind(&bind_adr).await;
    assert_eq!(true, listener_result.is_ok());
    let listener = listener_result.unwrap();
    tokio::task::spawn(test_tcp_server_accept(listener, 5));

    let max_cons = 5;
    let raw_manager = Manager::new(Destination::new("127.0.0.1".to_owned(), port), max_cons);
    let manager_arc = std::sync::Arc::new(raw_manager);
    Manager::start(manager_arc.clone()).await;

    assert_eq!(5, manager_arc.available_connections().await);
}

#[tokio::test]
async fn manager_get() {
    let port = 8081;

    let bind_adr = format!("127.0.0.1:{}", port);
    let listener_result = tokio::net::TcpListener::bind(&bind_adr).await;
    assert_eq!(true, listener_result.is_ok());
    let listener = listener_result.unwrap();
    tokio::task::spawn(test_tcp_server_accept(listener, 1));

    let max_cons = 5;
    let raw_manager = Manager::new(Destination::new("127.0.0.1".to_owned(), port), max_cons);

    let get_result = raw_manager.get().await;
    assert_eq!(true, get_result.is_ok());
}

#[tokio::test]
async fn manager_return_drop_read_con_first() {
    let port = 8082;
    let max_cons: usize = 5;

    let bind_adr = format!("127.0.0.1:{}", port);
    let listener_result = tokio::net::TcpListener::bind(&bind_adr).await;
    assert_eq!(true, listener_result.is_ok());
    let listener = listener_result.unwrap();
    tokio::task::spawn(test_tcp_server_accept(listener, max_cons as u32));

    let raw_manager = Manager::new(Destination::new("127.0.0.1".to_owned(), port), max_cons);
    let manager_arc = std::sync::Arc::new(raw_manager);
    Manager::start(manager_arc.clone()).await;

    assert_eq!(max_cons, manager_arc.available_connections().await);

    let get_result = manager_arc.get().await;
    assert_eq!(true, get_result.is_ok());

    assert_eq!(max_cons - 1, manager_arc.available_connections().await);
    // Check that no reads were previously marked as needing recovery
    let tmp_reads = manager_arc.recovered_reads.lock().await;
    assert_eq!(0, tmp_reads.len());
    drop(tmp_reads);

    let (read, write) = get_result.unwrap();
    drop(read);

    // To allow for the collect task to recv the drop message
    tokio::task::yield_now().await;

    // Check that the one reader were marked as needing recovery
    let tmp_reads = manager_arc.recovered_reads.lock().await;
    assert_eq!(1, tmp_reads.len());
    drop(tmp_reads);

    drop(write);

    // To allow for the collect task to recv the drop message
    tokio::task::yield_now().await;

    // Checking that they are now again available
    // and not stored anymore
    assert_eq!(max_cons, manager_arc.available_connections().await);
}

#[tokio::test]
async fn manager_return_drop_write_con_first() {
    let port = 8083;
    let max_cons: usize = 5;

    let bind_adr = format!("127.0.0.1:{}", port);
    let listener_result = tokio::net::TcpListener::bind(&bind_adr).await;
    assert_eq!(true, listener_result.is_ok());
    let listener = listener_result.unwrap();
    tokio::task::spawn(test_tcp_server_accept(listener, max_cons as u32));

    let raw_manager = Manager::new(Destination::new("127.0.0.1".to_owned(), port), max_cons);
    let manager_arc = std::sync::Arc::new(raw_manager);
    Manager::start(manager_arc.clone()).await;

    assert_eq!(max_cons, manager_arc.available_connections().await);

    let get_result = manager_arc.get().await;
    assert_eq!(true, get_result.is_ok());

    assert_eq!(max_cons - 1, manager_arc.available_connections().await);
    // Check that no reads were previously marked as needing recovery
    let tmp_reads = manager_arc.recovered_reads.lock().await;
    assert_eq!(0, tmp_reads.len());
    drop(tmp_reads);

    let (read, write) = get_result.unwrap();
    drop(write);

    // To allow for the collect task to recv the drop message
    tokio::task::yield_now().await;

    // Check that the one reader were marked as needing recovery
    let tmp_writes = manager_arc.recovered_writes.lock().await;
    assert_eq!(1, tmp_writes.len());
    drop(tmp_writes);

    drop(read);

    // To allow for the collect task to recv the drop message
    tokio::task::yield_now().await;

    // Checking that they are now again available
    // and not stored anymore
    assert_eq!(max_cons, manager_arc.available_connections().await);
}
