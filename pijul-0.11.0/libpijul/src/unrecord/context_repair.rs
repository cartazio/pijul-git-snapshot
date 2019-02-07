use Result;
use backend::*;
use rand;
use std::collections::HashMap;

impl<'env, T: rand::Rng> MutTxn<'env, T> {
    fn collect_up_context_repair(
        &self,
        branch: &Branch,
        key: Key<PatchId>,
        patch_id: PatchId,
        edges: &mut HashMap<Key<PatchId>, Edge>,
    ) {
        debug!("collect up {:?}", key);
        let start_flag = EdgeFlags::PARENT_EDGE | EdgeFlags::PSEUDO_EDGE;
        for v in self.iter_adjacent(branch, key, start_flag, start_flag | EdgeFlags::FOLDER_EDGE)
            .take_while(|v| { v.introduced_by == patch_id }) {
            if !edges.contains_key(&key) {
                edges.insert(key.to_owned(), v.to_owned());
                self.collect_up_context_repair(branch, v.dest, patch_id, edges)
            }
        }
    }

    fn collect_down_context_repair(
        &self,
        branch: &Branch,
        key: Key<PatchId>,
        patch_id: PatchId,
        edges: &mut HashMap<Key<PatchId>, Edge>,
    ) {
        debug!("collect down {:?}", key);
        for v in self.iter_adjacent(branch, key, EdgeFlags::PSEUDO_EDGE, EdgeFlags::PSEUDO_EDGE | EdgeFlags::FOLDER_EDGE)
            .take_while(|v| { v.introduced_by == patch_id }) {
            if !edges.contains_key(&key) {
                edges.insert(key.to_owned(), v.to_owned());

                self.collect_down_context_repair(branch, v.dest, patch_id, edges)
            }
        }
    }

    pub fn remove_up_context_repair(
        &mut self,
        branch: &mut Branch,
        key: Key<PatchId>,
        patch_id: PatchId,
        edges: &mut HashMap<Key<PatchId>, Edge>,
    ) -> Result<()> {
        self.collect_up_context_repair(branch, key, patch_id, edges);
        for (mut k, mut v) in edges.drain() {
            debug!("remove {:?} {:?}", k, v);

            self.del_edge_both_dirs(branch, k, v)?;
        }

        Ok(())
    }

    pub fn remove_down_context_repair(
        &mut self,
        branch: &mut Branch,
        key: Key<PatchId>,
        patch_id: PatchId,
        edges: &mut HashMap<Key<PatchId>, Edge>,
    ) -> Result<()> {
        self.collect_down_context_repair(branch, key, patch_id, edges);
        for (mut k, mut v) in edges.drain() {
            self.del_edge_both_dirs(branch, k, v)?;
        }

        Ok(())
    }
}
