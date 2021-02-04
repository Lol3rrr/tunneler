use dashmap::DashMap;

pub struct Connections<T> {
    connections: DashMap<u32, T>,
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

    pub fn get(&self, id: u32) -> Option<dashmap::mapref::one::Ref<u32, T>> {
        self.connections.get(&id)
    }

    pub fn get_mut(&self, id: u32) -> Option<dashmap::mapref::one::RefMut<u32, T>> {
        self.connections.get_mut(&id)
    }

    pub fn set(&self, id: u32, con: T) {
        self.connections.insert(id, con);
    }

    pub fn remove(&self, id: u32) -> Option<(u32, T)> {
        self.connections.remove(&id)
    }
}
