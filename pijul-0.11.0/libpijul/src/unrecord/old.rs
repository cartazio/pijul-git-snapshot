
    fn reconnect_deleted_folder_nodes(
        &mut self,
        branch: &mut Branch,
        deleted_nodes: &[Key<PatchId>],
    ) -> Result<()> {
        debug!("reconnect_deleted_folder_nodes");
        let mut find_alive = FindAlive::new();
        let mut alive_ancestors = Vec::new();
        // find alive ancestors of the deleted nodes.
        unimplemented!();
        for &c in deleted_nodes {
            debug!("down_context c = {:?}", c);
            if !self.is_alive(branch, c) {
                find_alive.clear();
                find_alive.push(c);
                self.find_alive_ancestors(&mut find_alive, branch, &mut alive_ancestors);
            }
        }
        // find alive descendants of the deleted nodes.
        let mut alive_descendants = Vec::new();
        if !alive_ancestors.is_empty() {
            for &c in deleted_nodes {
                debug!("down_context c = {:?}", c);
                if !self.is_alive(branch, c) {
                    find_alive.clear();
                    find_alive.push(c);
                    self.find_alive_descendants(&mut find_alive, branch, &mut alive_descendants);
                }
            }
        }
        debug!(
            "ancestors = {:?}, descendants = {:?}",
            alive_ancestors,
            alive_descendants
        );
        for ancestor in alive_ancestors.iter() {
            for descendant in alive_descendants.iter() {
                let mut edge = Edge::zero(EdgeFlags::PSEUDO_EDGE | EdgeFlags::FOLDER_EDGE);
                edge.dest = *descendant;
                debug!("adding {:?} -> {:?}", ancestor, edge);
                self.put_nodes_with_rev(branch, *ancestor, edge)?;
            }
        }
        Ok(())
    }
