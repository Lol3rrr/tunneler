use crate::server::client::Client;

pub struct ClientManager {
    index: std::sync::atomic::AtomicU64,
    clients: std::sync::Mutex<Vec<Client>>,
}

impl Default for ClientManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientManager {
    /// Creates a new empty Client-Manager
    pub fn new() -> ClientManager {
        ClientManager {
            index: std::sync::atomic::AtomicU64::new(0),
            clients: std::sync::Mutex::new(Vec::new()),
        }
    }

    // TODO implement a proper client selection instead of just returning
    // the first one
    pub fn get(&self) -> Option<Client> {
        let clients_data = self.clients.lock().unwrap();
        let index =
            self.index.load(std::sync::atomic::Ordering::SeqCst) as usize % clients_data.len();
        let client = match clients_data.get(index) {
            Some(c) => c.clone(),
            None => {
                return None;
            }
        };
        drop(clients_data);

        self.index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        Some(client)
    }

    /// Adds a new client connection to the List of connections
    ///
    /// Params:
    /// * client: The client to add
    pub fn add(&self, client: Client) {
        let mut clients_data = self.clients.lock().unwrap();
        clients_data.push(client);
        drop(clients_data);
    }

    /// This is used to remove a client connection again
    ///
    /// Params:
    /// * id: The ID of the connection to remove
    pub fn remove_con(&self, id: u32) {
        let mut client_data = self.clients.lock().unwrap();
        let mut remove_index: Option<usize> = None;
        for (index, client) in client_data.iter().enumerate() {
            if client.get_id() == id {
                remove_index = Some(index);
                break;
            }
        }

        match remove_index {
            None => {}
            Some(i) => {
                client_data.remove(i);
            }
        };

        drop(client_data);
    }
}
