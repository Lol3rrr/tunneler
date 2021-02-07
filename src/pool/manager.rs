use crate::pool::connection::{ReadConnection, WriteConnection};
use crate::Destination;

use log::{debug, error};

use tokio::sync::broadcast::{self, Sender};

use rand::Rng;

type NotifyReadMessage = (u64, bool, std::sync::Arc<tokio::net::tcp::OwnedReadHalf>);
type NotifyWriteMessage = (u64, bool, std::sync::Arc<tokio::net::tcp::OwnedWriteHalf>);

pub struct Manager {
    /// The Destination where each connetion should connect to
    dest: Destination,
    /// The maximum number of connects held by this pool
    max_cons: usize,
    /// All available Connection-Pairs
    avail_cons: tokio::sync::Mutex<Vec<(ReadConnection, WriteConnection)>>,
    /// The Transmit-Half for all notifcations regarding the Read-Parts
    notify_read_tx: Sender<NotifyReadMessage>,
    /// The Transmit-Half for all notifcations regarding the Write-Parts
    notify_write_tx: Sender<NotifyWriteMessage>,
    /// All the Recovered but not yet completly recovered connections are stored here
    recovered_reads: tokio::sync::Mutex<Vec<(u64, tokio::net::tcp::OwnedReadHalf)>>,
    recovered_writes: tokio::sync::Mutex<Vec<(u64, tokio::net::tcp::OwnedWriteHalf)>>,
    /// All the IDs of connections where one Half was returned as invalid
    invalid_ids: tokio::sync::Mutex<Vec<u64>>,
}

impl Manager {
    pub fn new(dest: Destination, max_cons: usize) -> Self {
        let (read_tx, _) = broadcast::channel(10);
        let (write_tx, _) = broadcast::channel(10);

        Self {
            dest,
            max_cons,
            avail_cons: tokio::sync::Mutex::new(Vec::with_capacity(max_cons)),
            notify_read_tx: read_tx,
            notify_write_tx: write_tx,
            recovered_reads: tokio::sync::Mutex::new(Vec::new()),
            recovered_writes: tokio::sync::Mutex::new(Vec::new()),
            invalid_ids: tokio::sync::Mutex::new(Vec::new()),
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

    fn find_id(id: u64, arr: &[u64]) -> Option<usize> {
        for (index, tmp_id) in arr.iter().enumerate() {
            if id == *tmp_id {
                return Some(index);
            }
        }

        None
    }

    /// Actually handles all the recover logic needed for both the Reader- and Writer-Halves
    async fn recover<C, T>(
        id: u64,
        is_valid: bool,
        con: C,
        same_recovered: &mut Vec<(u64, C)>,
        other_recovered: &mut Vec<(u64, T)>,
        errors: &mut Vec<u64>,
    ) -> Option<(C, T)> {
        // Handles all cases where the other Half has already been returned as valid
        if let Some(index) = Manager::find_con(id, &other_recovered) {
            let (_, other_con) = other_recovered.remove(index);

            if !is_valid {
                return None;
            }

            return Some((con, other_con));
        }

        // If an entry with the same id was made to the errored list
        // remove it and drop the current connection as well
        if let Some(index) = Manager::find_id(id, &errors) {
            errors.remove(index);
            return None;
        }

        // Handles all the cases the other half has not yet been returned or
        // was returned as invalid and this half is not valid
        if !is_valid {
            errors.push(id);

            return None;
        }

        // If this half is valid and the other half was not returned yet, simply
        // add it the list of returned and valid halves
        same_recovered.push((id, con));

        None
    }

    /// Takes the Mutex for the Read-Receiver and then enters
    /// an infinite loop that always tries to receive a Read
    /// Half that will then be made available again once the
    /// Write Half will also be received by another process
    async fn recover_read_loop(arc: std::sync::Arc<Self>) {
        let mut rx = arc.notify_read_tx.subscribe();
        loop {
            let (id, is_valid, raw_read_con) = match rx.recv().await {
                Ok(m) => m,
                Err(e) => {
                    error!("Reading Pool-Read-Receiver: {}", e);
                    return;
                }
            };

            let read_con = std::sync::Arc::try_unwrap(raw_read_con).unwrap();

            let mut tmp_reads = arc.recovered_reads.lock().await;
            let mut tmp_writes = arc.recovered_writes.lock().await;
            let mut errored_ids = arc.invalid_ids.lock().await;

            let mut avail_cons = arc.avail_cons.lock().await;

            match Manager::recover(
                id,
                is_valid,
                read_con,
                &mut tmp_reads,
                &mut tmp_writes,
                &mut errored_ids,
            )
            .await
            {
                Some((read, write)) => {
                    let pool_read_con = ReadConnection::new(id, read, arc.notify_read_tx.clone());
                    let pool_write_con =
                        WriteConnection::new(id, write, arc.notify_write_tx.clone());
                    avail_cons.push((pool_read_con, pool_write_con));

                    debug!("Recovered Connection");
                }
                None => {}
            };
        }
    }

    /// Takes the Mutex for the Write-Receiver and then enters
    /// an infinite loop that always tries to receive a Writer
    /// Half that will then be made available again once the
    /// Read Half will also be received by another process
    async fn recover_write_loop(arc: std::sync::Arc<Self>) {
        let mut rx = arc.notify_write_tx.subscribe();
        loop {
            let (id, is_valid, raw_write_con) = match rx.recv().await {
                Ok(m) => m,
                Err(e) => {
                    error!("Reading Pool-Write-Receiver: {}", e);
                    return;
                }
            };

            let write_con = std::sync::Arc::try_unwrap(raw_write_con).unwrap();

            let mut tmp_reads = arc.recovered_reads.lock().await;
            let mut tmp_writes = arc.recovered_writes.lock().await;
            let mut errored_ids = arc.invalid_ids.lock().await;

            let mut avail_cons = arc.avail_cons.lock().await;

            match Manager::recover(
                id,
                is_valid,
                write_con,
                &mut tmp_writes,
                &mut tmp_reads,
                &mut errored_ids,
            )
            .await
            {
                Some((write, read)) => {
                    let pool_read_con = ReadConnection::new(id, read, arc.notify_read_tx.clone());
                    let pool_write_con =
                        WriteConnection::new(id, write, arc.notify_write_tx.clone());
                    avail_cons.push((pool_read_con, pool_write_con));

                    debug!("Recovered Connection");
                }
                None => {}
            };
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
    let port = 9080;

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
    let port = 9081;

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
    let port = 9082;
    let max_cons: usize = 5;

    let bind_adr = format!("127.0.0.1:{}", port);
    let listener_result = tokio::net::TcpListener::bind(&bind_adr).await;
    assert_eq!(true, listener_result.is_ok());
    let listener = listener_result.unwrap();
    tokio::task::spawn(test_tcp_server_accept(listener, max_cons as u32));

    let raw_manager = Manager::new(Destination::new("127.0.0.1".to_owned(), port), max_cons);
    let manager_arc = std::sync::Arc::new(raw_manager);
    Manager::start(manager_arc.clone()).await;

    tokio::task::yield_now().await;

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
    let port = 9083;
    let max_cons: usize = 5;

    let bind_adr = format!("127.0.0.1:{}", port);
    let listener_result = tokio::net::TcpListener::bind(&bind_adr).await;
    assert_eq!(true, listener_result.is_ok());
    let listener = listener_result.unwrap();
    tokio::task::spawn(test_tcp_server_accept(listener, max_cons as u32));

    let raw_manager = Manager::new(Destination::new("127.0.0.1".to_owned(), port), max_cons);
    let manager_arc = std::sync::Arc::new(raw_manager);
    Manager::start(manager_arc.clone()).await;

    tokio::task::yield_now().await;

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

#[tokio::test]
async fn manager_return_drop_invalid_valid() {
    let port = 9084;
    let max_cons: usize = 5;

    let bind_adr = format!("127.0.0.1:{}", port);
    let listener_result = tokio::net::TcpListener::bind(&bind_adr).await;
    assert_eq!(true, listener_result.is_ok());
    let listener = listener_result.unwrap();
    tokio::task::spawn(test_tcp_server_accept(listener, max_cons as u32));

    let raw_manager = Manager::new(Destination::new("127.0.0.1".to_owned(), port), max_cons);
    let manager_arc = std::sync::Arc::new(raw_manager);
    Manager::start(manager_arc.clone()).await;

    tokio::task::yield_now().await;

    assert_eq!(max_cons, manager_arc.available_connections().await);

    let get_result = manager_arc.get().await;
    assert_eq!(true, get_result.is_ok());

    assert_eq!(max_cons - 1, manager_arc.available_connections().await);

    let (mut read, write) = get_result.unwrap();
    read.invalidate();
    drop(read);

    // To allow for the collect task to recv the drop message
    tokio::task::yield_now().await;

    // Check that the one reader were marked as needing recovery

    drop(write);

    // To allow for the collect task to recv the drop message
    tokio::task::yield_now().await;

    // Checking that they are not made available again
    // and not stored anywhere anymore
    assert_eq!(max_cons - 1, manager_arc.available_connections().await);
    // Checking that no more reads are marked as in recovery
    let tmp_reads = manager_arc.recovered_reads.lock().await;
    assert_eq!(0, tmp_reads.len());
    drop(tmp_reads);
    // Checking that no more reads are marked as in recovery
    let tmp_writes = manager_arc.recovered_writes.lock().await;
    assert_eq!(0, tmp_writes.len());
    drop(tmp_writes);
}

#[tokio::test]
async fn manager_return_drop_valid_invalid() {
    let port = 9085;
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

    let (read, mut write) = get_result.unwrap();
    drop(read);

    // To allow for the collect task to recv the drop message
    tokio::task::yield_now().await;

    // Check that the one reader were marked as needing recovery

    write.invalidate();
    drop(write);

    // To allow for the collect task to recv the drop message
    tokio::task::yield_now().await;

    // Checking that they are not made available again
    // and not stored anywhere anymore
    assert_eq!(max_cons - 1, manager_arc.available_connections().await);
    // Checking that no more reads are marked as in recovery
    let tmp_reads = manager_arc.recovered_reads.lock().await;
    assert_eq!(0, tmp_reads.len());
    drop(tmp_reads);
    // Checking that no more reads are marked as in recovery
    let tmp_writes = manager_arc.recovered_writes.lock().await;
    assert_eq!(0, tmp_writes.len());
    drop(tmp_writes);
}
