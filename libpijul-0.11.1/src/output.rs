use backend::*;
use patch::*;
use record::InodeUpdate;
use {Error, Result};
use graph;
use rand;
use std;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tempdir;

#[cfg(not(windows))]
use std::os::unix::fs::PermissionsExt;

#[cfg(not(windows))]
fn set_permissions(name: &Path, permissions: u16) -> Result<()> {
    let metadata = std::fs::metadata(&name)?;
    let mut current = metadata.permissions();
    debug!(
        "setting mode for {:?} to {:?} (currently {:?})",
        name, permissions, current
    );
    current.set_mode(permissions as u32);
    std::fs::set_permissions(name, current)?;
    Ok(())
}

#[cfg(windows)]
fn set_permissions(_name: &Path, _permissions: u16) -> Result<()> {
    Ok(())
}

#[derive(Debug)]
struct OutputItem {
    parent: Inode,
    meta: FileMetadata,
    key: Key<PatchId>,
    inode: Option<Inode>,
    is_zombie: bool,
    related: Related,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Related {
    No,
    Ancestor,
    Exact,
}

fn is_related(prefixes: &Prefixes, key: Key<PatchId>) -> Related {
    if prefixes.0.is_empty() {
        return Related::Exact;
    }
    for pref in prefixes.0.iter() {
        let mut is_first = true;
        for &p in pref {
            if p == key {
                if is_first {
                    return Related::Exact;
                } else {
                    return Related::Ancestor;
                }
            }
            is_first = false
        }
    }
    Related::No
}

impl<'env, T: rand::Rng> MutTxn<'env, T> {
    // Climb up the tree (using revtree).
    fn filename_of_inode(&self, inode: Inode, working_copy: &Path) -> Option<PathBuf> {
        let mut components = Vec::new();
        let mut current = inode;
        loop {
            match self.get_revtree(current) {
                Some(v) => {
                    components.push(v.basename.to_owned());
                    current = v.parent_inode.clone();
                    if current == ROOT_INODE {
                        break;
                    }
                }
                None => {
                    debug!("filename_of_inode: not in tree");
                    return None;
                }
            }
        }
        let mut working_copy = working_copy.to_path_buf();
        for c in components.iter().rev() {
            working_copy.push(c.as_small_str().as_str());
        }
        Some(working_copy)
    }

    /// Collect all the children of key `key` into `files`.
    fn collect_children(
        &mut self,
        branch: &Branch,
        path: &Path,
        key: Key<PatchId>,
        inode: Inode,
        base_path: &mut PathBuf,
        prefixes: &Prefixes,
        files: &mut HashMap<PathBuf, HashMap<Key<PatchId>, OutputItem>>,
    ) -> Result<()> {
        debug!("collect_children {:?}", base_path);
        let f = EdgeFlags::FOLDER_EDGE | EdgeFlags::PSEUDO_EDGE | EdgeFlags::EPSILON_EDGE;
        for b in self.iter_adjacent(&branch, key, EdgeFlags::empty(), f)
        {
            debug!("b={:?}", b);
            let cont_b = self.get_contents(b.dest).unwrap();
            let (_, b_key) = self.iter_nodes(&branch,
                                             Some((b.dest, Some(Edge::zero(EdgeFlags::FOLDER_EDGE)))))
                .next()
                .unwrap();
            let b_inode = self.get_revinodes(b_key.dest);

            // This is supposed to be a small string, so we can do
            // as_slice.
            if cont_b.as_slice().len() < 2 {
                error!("cont_b {:?} b.dest {:?}", cont_b, b.dest);
                return Err(Error::WrongFileHeader(b.dest));
            }
            let (perms, basename) = cont_b.as_slice().split_at(2);

            let perms = FileMetadata::from_contents(perms);
            let basename = std::str::from_utf8(basename).unwrap();
            debug!("filename: {:?} {:?}", perms, basename);
            let name = path.join(basename);
            let related = is_related(&prefixes, b_key.dest);
            debug!("related {:?} = {:?}", base_path, related);
            if related != Related::No {
                let v = files.entry(name).or_insert(HashMap::new());
                if v.get(&b.dest).is_none() {
                    let is_zombie = {
                        let f = EdgeFlags::FOLDER_EDGE | EdgeFlags::PARENT_EDGE | EdgeFlags::DELETED_EDGE;
                        self.iter_adjacent(&branch, b_key.dest, f, f)
                            .next()
                            .is_some()
                    };
                    debug!("is_zombie = {:?}", is_zombie);
                    v.insert(
                        b.dest,
                        OutputItem {
                            parent: inode,
                            meta: perms,
                            key: b_key.dest,
                            inode: b_inode,
                            is_zombie,
                            related,
                        },
                    );
                }
            }
        }
        Ok(())
    }

    /// Collect names of files with conflicts
    ///
    /// As conflicts have an internal representation, it can be determined
    /// exactly which files contain conflicts.
    pub fn list_conflict_files(
        &mut self,
        branch_name: &str,
        prefixes: &[&str],
    ) -> Result<Vec<PathBuf>> {
        let mut files = HashMap::new();
        let mut next_files = HashMap::new();
        let branch = self.open_branch(branch_name)?;
        let mut base_path = PathBuf::new();
        let prefixes = prefixes.to_prefixes(self, &branch);
        self.collect_children(
            &branch,
            "".as_ref(),
            ROOT_KEY,
            ROOT_INODE,
            &mut base_path,
            &prefixes,
            &mut files,
        )?;

        let mut ret = vec![];
        let mut forward = Vec::new();
        while !files.is_empty() {
            next_files.clear();
            for (a, b) in files.drain() {
                for (_, output_item) in b {
                    // (_, meta, inode_key, inode, is_zombie)
                    // Only bother with existing files
                    if let Some(inode) = output_item.inode {
                        if output_item.is_zombie {
                            ret.push(a.clone())
                        }
                        if output_item.meta.is_dir() {
                            self.collect_children(
                                &branch,
                                &a,
                                output_item.key,
                                inode,
                                &mut base_path,
                                &prefixes,
                                &mut next_files,
                            )?;
                        } else {
                            let mut graph = self.retrieve(&branch, output_item.key);
                            let mut buf = graph::Writer::new(std::io::sink());
                            if self.output_file(&branch, &mut buf, &mut graph, &mut forward)? {
                                ret.push(a.clone())
                            }
                        }
                    }
                }
            }
            std::mem::swap(&mut files, &mut next_files);
        }
        Ok(ret)
    }

    fn make_conflicting_name(&self, name: &mut PathBuf, name_key: Key<PatchId>) {
        let basename = {
            let basename = name.file_name().unwrap().to_string_lossy();
            format!("{}.{}", basename, &name_key.patch.to_base58())
        };
        name.set_file_name(&basename);
    }

    fn output_alive_files(
        &mut self,
        branch: &mut Branch,
        prefixes: &Prefixes,
        working_copy: &Path,
    ) -> Result<()> {
        debug!("working copy {:?}", working_copy);
        let mut files = HashMap::new();
        let mut next_files = HashMap::new();
        let mut base_path = PathBuf::new();
        self.collect_children(
            branch,
            "".as_ref(),
            ROOT_KEY,
            ROOT_INODE,
            &mut base_path,
            prefixes,
            &mut files,
        )?;

        let mut done = HashSet::new();

        while !files.is_empty() {
            debug!("files {:?}", files);
            next_files.clear();
            for (a, b) in files.drain() {
                let b_len = b.len();
                for (name_key, output_item) in b {
                    // (parent_inode, meta, inode_key, inode, is_zombie)
                    /*let has_several_names = {
                        let e = Edge::zero(EdgeFlags::PARENT_EDGE | EdgeFlags::FOLDER_EDGE);
                        let mut it = self.iter_nodes(branch, Some((inode_key, Some(&e))))
                            .take_while(|&(k, v)| {
                                k == inode_key && v.flag|EdgeFlags::PSEUDO_EDGE == e.flag|EdgeFlags::PSEUDO_EDGE
                            });
                        it.next();
                        it.next().is_some()
                    };*/
                    if !done.insert(output_item.key) {
                        debug!("already done {:?}", output_item.key);
                        continue;
                    }

                    let mut name = if b_len > 1
                    /*|| has_several_names*/
                    {
                        // debug!("b_len = {:?}, has_several_names {:?}", b_len, has_several_names);
                        let mut name = a.clone();
                        self.make_conflicting_name(&mut name, name_key);
                        Cow::Owned(name)
                    } else {
                        Cow::Borrowed(&a)
                    };
                    let file_name = name.file_name().unwrap().to_string_lossy();
                    base_path.push(Path::new(file_name.as_ref()));
                    let file_id = OwnedFileId {
                        parent_inode: output_item.parent,
                        basename: SmallString::from_str(&file_name),
                    };
                    let working_copy_name = working_copy.join(name.as_ref());

                    let status = if output_item.is_zombie {
                        FileStatus::Zombie
                    } else {
                        FileStatus::Ok
                    };

                    let inode = if let Some(inode) = output_item.inode {
                        // If the file already exists, find its
                        // current name and rename it if that name
                        // is different.
                        if let Some(ref current_name) = self.filename_of_inode(inode, "".as_ref()) {
                            if current_name != name.as_ref() {
                                let current_name = working_copy.join(current_name);
                                debug!("renaming {:?} to {:?}", current_name, working_copy_name);
                                let parent = self.get_revtree(inode).unwrap().to_owned();
                                self.del_revtree(inode, None)?;
                                self.del_tree(&parent.as_file_id(), None)?;

                                debug!("file_id: {:?}", file_id);
                                if let Some(p) = working_copy_name.parent() {
                                    std::fs::create_dir_all(p)?
                                }
                                if let Err(e) = std::fs::rename(&current_name, &working_copy_name) {
                                    error!(
                                        "while renaming {:?} to {:?}: {:?}",
                                        current_name, working_copy_name, e
                                    )
                                }
                            }
                        }
                        self.put_tree(&file_id.as_file_id(), inode)?;
                        self.put_revtree(inode, &file_id.as_file_id())?;
                        // If the file had been marked for deletion, remove that mark.
                        if let Some(header) = self.get_inodes(inode) {
                            debug!("header {:?}", header);
                            let mut header = header.to_owned();
                            header.status = status;
                            self.replace_inodes(inode, header)?;
                        } else {
                            let header = FileHeader {
                                key: output_item.key,
                                metadata: output_item.meta,
                                status,
                            };
                            debug!("no header {:?}", header);
                            self.replace_inodes(inode, header)?;
                            self.replace_revinodes(output_item.key, inode)?;
                        }
                        inode
                    } else {
                        // Else, create new inode.
                        let inode = self.create_new_inode();
                        let file_header = FileHeader {
                            key: output_item.key,
                            metadata: output_item.meta,
                            status,
                        };
                        self.replace_inodes(inode, file_header)?;
                        self.replace_revinodes(output_item.key, inode)?;
                        debug!("file_id: {:?}", file_id);
                        self.put_tree(&file_id.as_file_id(), inode)?;
                        self.put_revtree(inode, &file_id.as_file_id())?;
                        inode
                    };
                    if output_item.meta.is_dir() {
                        // This is a directory, register it in inodes/trees.
                        std::fs::create_dir_all(&working_copy_name)?;
                        if let Related::Exact = output_item.related {
                            self.collect_children(
                                branch,
                                &name,
                                output_item.key,
                                inode,
                                &mut base_path,
                                &Prefixes(Vec::new()),
                                &mut next_files,
                            )?
                        } else {
                            self.collect_children(
                                branch,
                                &name,
                                output_item.key,
                                inode,
                                &mut base_path,
                                prefixes,
                                &mut next_files,
                            )?
                        }
                    } else {
                        // Output file.
                        debug!(
                            "creating file {:?}, key {:?} {:?}",
                            &name, output_item.key, working_copy_name
                        );
                        let mut f = graph::Writer::new(std::fs::File::create(&working_copy_name).unwrap());
                        debug!("done");
                        let mut l = self.retrieve(branch, output_item.key);
                        /*{
                            let mut w = working_copy_name.to_path_buf();
                            w.set_extension("graph");
                            let mut f = std::fs::File::create(&w).unwrap();
                            l.debug(self, branch, false, false, &mut f).unwrap();
                        }*/
                        let mut forward = Vec::new();
                        self.output_file(branch, &mut f, &mut l, &mut forward)?;
                        // self.remove_redundant_edges(branch, &forward)?
                    }
                    base_path.pop();
                    set_permissions(&working_copy_name, output_item.meta.permissions())?
                }
            }
            std::mem::swap(&mut files, &mut next_files);
        }
        Ok(())
    }

    fn output_repository_assuming_no_pending_patch(
        &mut self,
        prefixes: &Prefixes,
        branch: &mut Branch,
        working_copy: &Path,
        pending_patch_id: PatchId,
    ) -> Result<()> {
        debug!(
            "inodes: {:?}",
            self.iter_inodes(None)
                .map(|(u, v)| (u.to_owned(), v.to_owned()))
                .collect::<Vec<_>>()
        );
        // Now, garbage collect dead inodes.
        let dead: Vec<_> = self.iter_tree(None)
            .filter_map(|(k, v)| {
                debug!("{:?} {:?}", k, v);
                if let Some(key) = self.get_inodes(v) {
                    if key.key.patch == pending_patch_id || self.is_alive_or_zombie(branch, key.key)
                    {
                        // Don't delete.
                        None
                    } else {
                        Some((k.to_owned(), v, self.filename_of_inode(v, working_copy)))
                    }
                } else {
                    debug!("not in inodes");
                    Some((k.to_owned(), v, None))
                }
            })
            .collect();
        debug!("dead: {:?}", dead);

        // Now, "kill the deads"
        for (ref parent, inode, ref name) in dead {
            self.remove_inode_rec(inode)?;
            debug!("removed");
            if let Some(ref name) = *name {
                debug!("deleting {:?}", name);
                if let Ok(meta) = fs::metadata(name) {
                    if let Err(e) = if meta.is_dir() {
                        fs::remove_dir_all(name)
                    } else {
                        fs::remove_file(name)
                    } {
                        error!("while deleting {:?}: {:?}", name, e);
                    }
                }
            } else {
                self.del_tree(&parent.as_file_id(), Some(inode))?;
                self.del_revtree(inode, Some(&parent.as_file_id()))?;
            }
        }
        debug!("done deleting dead files");
        // Then output alive files. This has to be done *after*
        // removing files, because we a file removed might have the
        // same name as a file added without there being a conflict
        // (depending on the relation between the two patches).
        self.output_alive_files(branch, prefixes, working_copy)?;
        debug!("done raw_output_repository");
        Ok(())
    }

    fn remove_inode_rec(&mut self, inode: Inode) -> Result<()> {
        // Remove the inode from inodes/revinodes.
        let mut to_kill = vec![inode];
        while let Some(inode) = to_kill.pop() {
            debug!("kill dead {:?}", inode.to_hex());
            let header = self.get_inodes(inode).map(|x| x.to_owned());
            if let Some(header) = header {
                self.del_inodes(inode, None)?;
                self.del_revinodes(header.key, None)?;
                let mut kills = Vec::new();
                // Remove the inode from tree/revtree.
                for (k, v) in self.iter_revtree(Some((inode, None)))
                    .take_while(|&(k, _)| k == inode)
                {
                    kills.push((k.clone(), v.to_owned()))
                }
                for &(k, ref v) in kills.iter() {
                    self.del_tree(&v.as_file_id(), Some(k))?;
                    self.del_revtree(k, Some(&v.as_file_id()))?;
                }
                // If the dead is a directory, remove its descendants.
                let inode_fileid = OwnedFileId {
                    parent_inode: inode.clone(),
                    basename: SmallString::from_str(""),
                };
                to_kill.extend(
                    self.iter_tree(Some((&inode_fileid.as_file_id(), None)))
                        .take_while(|&(ref k, _)| k.parent_inode == inode)
                        .map(|(_, v)| v.to_owned()),
                )
            }
        }
        Ok(())
    }

    pub fn output_repository(
        &mut self,
        branch: &mut Branch,
        working_copy: &Path,
        prefixes: &Prefixes,
        pending: &Patch,
        local_pending: &HashSet<InodeUpdate>,
    ) -> Result<()> {
        debug!("begin output repository");

        debug!("applying pending patch");
        let tempdir = tempdir::TempDir::new("pijul")?;
        let hash = pending.save(tempdir.path(), None)?;
        let internal =
            self.apply_local_patch(branch, working_copy, &hash, pending, local_pending, true)?;

        debug!("applied");

        // let prefixes = prefixes.to_prefixes(&self, &branch);
        debug!("prefixes {:?}", prefixes);
        self.output_repository_assuming_no_pending_patch(
            &prefixes,
            branch,
            working_copy,
            internal,
        )?;

        debug!("unrecording pending patch");
        self.unrecord(branch, internal, pending)?;
        Ok(())
    }

    pub fn output_repository_no_pending(
        &mut self,
        branch: &mut Branch,
        working_copy: &Path,
        prefixes: &Prefixes,
    ) -> Result<()> {
        debug!("begin output repository {:?}", prefixes);

        // let prefixes = prefixes.iter().flat_map(|pref| self.prefix_keys(&branch, pref)).collect::<Vec<_>>();
        debug!("prefixes {:?}", prefixes);
        self.output_repository_assuming_no_pending_patch(
            &prefixes,
            branch,
            working_copy,
            ROOT_PATCH_ID,
        )?;
        Ok(())
    }

    pub(crate) fn output_partials(&mut self, branch_name: &str, prefixes: &Prefixes) -> Result<()> {
        for p in prefixes.0.iter() {
            self.put_partials(branch_name, p[0])?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Prefixes(Vec<Vec<Key<PatchId>>>);

impl Prefixes {
    pub fn empty() -> Self {
        Prefixes(Vec::new())
    }
}

pub trait ToPrefixes {
    fn to_prefixes<T>(&self, txn: &MutTxn<T>, branch: &Branch) -> Prefixes;
}

impl<'a> ToPrefixes for &'a [&'a str] {
    fn to_prefixes<T>(&self, txn: &MutTxn<T>, branch: &Branch) -> Prefixes {
        Prefixes(
            self.iter()
                .flat_map(|pref| txn.prefix_keys(&branch, pref))
                .collect(),
        )
    }
}

impl<'a> ToPrefixes for &'a [Inode] {
    fn to_prefixes<T>(&self, txn: &MutTxn<T>, _: &Branch) -> Prefixes {
        Prefixes(
            self.iter()
                .map(|pref| {
                    let mut result = Vec::new();
                    let mut current = *pref;
                    loop {
                        if current == ROOT_INODE {
                            result.push(ROOT_KEY);
                            break;
                        }
                        result.push(txn.get_inodes(current).unwrap().key);
                        match txn.get_revtree(current) {
                            Some(v) => current = v.parent_inode.clone(),
                            None => break,
                        }
                    }
                    result
                })
                .collect(),
        )
    }
}
