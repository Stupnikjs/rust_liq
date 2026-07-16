use std::collections::HashMap;
use alloy_primitives::FixedBytes;
use crate::swap::PoolEdge;
pub struct SwapRoute {

}

pub struct RouteCache {
    pub graph: HashMap<FixedBytes<32>, PoolEdge>,
}

impl RouteCache {
    pub fn new() -> Self {
        RouteCache {

            graph: HashMap::new(),
        }
    }

    pub fn insert_edge(&mut self, key: FixedBytes<32>, edge: PoolEdge) {
        self.graph.insert(key, edge);
    }

    pub fn get_edge(&self, key: &FixedBytes<32>) -> Option<&PoolEdge> {
        self.graph.get(key)
    }
}