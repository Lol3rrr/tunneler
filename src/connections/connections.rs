use dashmap::DashMap;

pub struct Connections<T> {
    connections: std::sync::Arc<DashMap<u32, T, fnv::FnvBuildHasher>>,
}

impl<T> Default for Connections<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Connections<T> {
    pub fn new() -> Connections<T> {
        Connections {
            connections: std::sync::Arc::new(DashMap::with_hasher(fnv::FnvBuildHasher::default())),
        }
    }

    #[inline(always)]
    pub fn get(&self, id: u32) -> Option<dashmap::mapref::one::Ref<u32, T, fnv::FnvBuildHasher>> {
        self.connections.get(&id)
    }

    #[inline(always)]
    pub fn get_mut(
        &self,
        id: u32,
    ) -> Option<dashmap::mapref::one::RefMut<u32, T, fnv::FnvBuildHasher>> {
        self.connections.get_mut(&id)
    }

    #[inline(always)]
    pub fn set(&self, id: u32, con: T) {
        self.connections.insert(id, con);
    }

    #[inline(always)]
    pub fn remove(&self, id: u32) -> Option<(u32, T)> {
        self.connections.remove(&id)
    }
}

impl<T> Clone for Connections<T> {
    fn clone(&self) -> Self {
        Self {
            connections: self.connections.clone(),
        }
    }
}
