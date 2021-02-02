use dashmap::DashMap;

pub struct Connections<T> {
    connections: DashMap<u32, std::sync::Arc<T>>,
}

impl<T> Default for Connections<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Connections<T> {
    pub fn new() -> Connections<T> {
        Connections {
            connections: DashMap::new(),
        }
    }

    pub fn get(&self, id: u32) -> Option<std::sync::Arc<T>> {
        match self.connections.get(&id) {
            None => None,
            Some(s) => Some(s.clone()),
        }
    }

    pub fn set(&self, id: u32, con: std::sync::Arc<T>) {
        self.connections.insert(id, con);
    }

    pub fn remove(&self, id: u32) -> Option<(u32, std::sync::Arc<T>)> {
        self.connections.remove(&id)
    }
}
