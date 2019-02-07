//! The data structure of the in-memory version of Pijul's main
//! datastructure, used to edit and organise it (for instance before a
//! record or before outputting a file).
use Result;
use backend::*;
use conflict;
use std::cmp::min;
use std::collections::{HashMap, HashSet};

use rand;
use std;

bitflags! {
    struct Flags: u8 {
        const LINE_HALF_DELETED = 4;
        const LINE_VISITED = 2;
        const LINE_ONSTACK = 1;
    }
}

/// The elementary datum in the representation of the repository state
/// at any given point in time. We need this structure (as opposed to
/// working directly on a branch) in order to add more data, such as
/// strongly connected component identifier, to each node.
#[derive(Debug)]
pub struct Line {
    /// The internal identifier of the line.
    pub key: Key<PatchId>,

    // The status of the line with respect to a dfs of
    // a graph it appears in. This is 0 or
    // `LINE_HALF_DELETED`.
    flags: Flags,
    children: usize,
    n_children: usize,
    index: usize,
    lowlink: usize,
    scc: usize,
}

impl Line {
    pub fn is_zombie(&self) -> bool {
        self.flags.contains(Flags::LINE_HALF_DELETED)
    }
}

/// A graph, representing the whole content of the repository state at
/// a point in time. The encoding is a "flat adjacency list", where
/// each vertex contains a index `children` and a number of children
/// `n_children`. The children of that vertex are then
/// `&g.children[children .. children + n_children]`.
#[derive(Debug)]
pub struct Graph {
    /// Array of all alive lines in the graph. Line 0 is a dummy line
    /// at the end, so that all nodes have a common successor
    pub lines: Vec<Line>,
    /// Edge + index of the line in the "lines" array above. "None"
    /// means "dummy line at the end", and corresponds to line number
    /// 0.
    children: Vec<(Option<Edge>, VertexId)>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
struct VertexId(usize);

const DUMMY_VERTEX: VertexId = VertexId(0);

impl std::ops::Index<VertexId> for Graph {
    type Output = Line;
    fn index(&self, idx: VertexId) -> &Self::Output {
        self.lines.index(idx.0)
    }
}
impl std::ops::IndexMut<VertexId> for Graph {
    fn index_mut(&mut self, idx: VertexId) -> &mut Self::Output {
        self.lines.index_mut(idx.0)
    }
}

use std::io::Write;

impl Graph {
    fn children(&self, i: VertexId) -> &[(Option<Edge>, VertexId)] {
        let ref line = self[i];
        &self.children[line.children..line.children + line.n_children]
    }

    fn child(&self, i: VertexId, j: usize) -> &(Option<Edge>, VertexId) {
        &self.children[self[i].children + j]
    }

    pub fn debug<W: Write, R, A: Transaction>(
        &self,
        txn: &GenericTxn<A, R>,
        branch: &Branch,
        add_others: bool,
        introduced_by: bool,
        mut w: W,
    ) -> std::io::Result<()> {
        writeln!(w, "digraph {{")?;
        let mut cache = HashMap::new();
        if add_others {
            for (line, i) in self.lines.iter().zip(0..) {
                cache.insert(line.key, i);
            }
        }
        let mut others = HashSet::new();
        for (line, i) in self.lines.iter().zip(0..) {
            let contents = {
                if let Some(c) = txn.get_contents(line.key) {
                    let c = c.into_cow();
                    if let Ok(c) = std::str::from_utf8(&c) {
                        c.split_at(std::cmp::min(50, c.len())).0.to_string()
                    } else {
                        "<INVALID>".to_string()
                    }
                } else {
                    "".to_string()
                }
            };
            let contents = format!("{:?}", contents);
            let contents = contents.split_at(contents.len() - 1).0.split_at(1).1;
            writeln!(
                w,
                "n_{}[label=\"{}: {}.{}: {}\"];",
                i,
                i,
                line.key.patch.to_base58(),
                line.key.line.to_hex(),
                contents
            )?;

            if add_others && !line.key.is_root() {
                for v in txn.iter_adjacent(branch, line.key, EdgeFlags::empty(), EdgeFlags::all()) {
                    if let Some(dest) = cache.get(&v.dest) {
                        writeln!(
                            w,
                            "n_{} -> n_{}[color=red,label=\"{:?}{}{}\"];",
                            i,
                            dest,
                            v.flag.bits(),
                            if introduced_by { " " } else { "" },
                            if introduced_by {
                                v.introduced_by.to_base58()
                            } else {
                                String::new()
                            }
                        )?;
                    } else {
                        if !others.contains(&v.dest) {
                            others.insert(v.dest);
                            writeln!(
                                w,
                                "n_{}[label=\"{}.{}\",color=red];",
                                v.dest.to_base58(),
                                v.dest.patch.to_base58(),
                                v.dest.line.to_hex()
                            )?;
                        }
                        writeln!(
                            w,
                            "n_{} -> n_{}[color=red,label=\"{:?}{}{}\"];",
                            i,
                            v.dest.to_base58(),
                            v.flag.bits(),
                            if introduced_by { " " } else { "" },
                            if introduced_by {
                                v.introduced_by.to_base58()
                            } else {
                                String::new()
                            }
                        )?;
                    }
                }
            }
            for &(ref edge, VertexId(j)) in
                &self.children[line.children..line.children + line.n_children]
            {
                if let Some(ref edge) = *edge {
                    writeln!(
                        w,
                        "n_{}->n_{}[label=\"{:?}{}{}\"];",
                        i,
                        j,
                        edge.flag.bits(),
                        if introduced_by { " " } else { "" },
                        if introduced_by {
                            edge.introduced_by.to_base58()
                        } else {
                            String::new()
                        }
                    )?
                } else {
                    writeln!(w, "n_{}->n_{}[label=\"none\"];", i, j)?
                }
            }
        }
        writeln!(w, "}}")?;
        Ok(())
    }
}

use sanakirja::value::Value;
/// A "line outputter" trait.
pub trait LineBuffer<'a, T: 'a + Transaction> {
    fn output_line(&mut self, key: &Key<PatchId>, contents: Value<'a, T>) -> Result<()>;

    fn output_conflict_marker(&mut self, s: &'a str) -> Result<()>;
    fn begin_conflict(&mut self) -> Result<()> {
        self.output_conflict_marker(conflict::START_MARKER)
    }
    fn conflict_next(&mut self) -> Result<()> {
        self.output_conflict_marker(conflict::SEPARATOR)
    }
    fn end_conflict(&mut self) -> Result<()> {
        self.output_conflict_marker(conflict::END_MARKER)
    }
}

pub struct Writer<W: std::io::Write> {
    pub w: W,
    new_line: bool,
}

impl<W: std::io::Write> Writer<W> {
    pub fn new(w: W) -> Self {
        Writer { w, new_line: true }
    }
}

impl<W: std::io::Write> std::ops::Deref for Writer<W> {
    type Target = W;
    fn deref(&self) -> &Self::Target {
        &self.w
    }
}

impl<W: std::io::Write> std::ops::DerefMut for Writer<W> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.w
    }
}

impl<'a, T: 'a + Transaction, W: std::io::Write> LineBuffer<'a, T> for Writer<W> {
    fn output_line(&mut self, k: &Key<PatchId>, c: Value<T>) -> Result<()> {
        let mut ends_with_newline = false;
        let mut is_empty = true;
        for chunk in c {
            debug!("output line {:?} {:?}", k, std::str::from_utf8(chunk));
            is_empty = is_empty && chunk.is_empty();
            ends_with_newline = chunk.ends_with(b"\n");
            self.w.write_all(chunk)?
        }
        if !is_empty {
            // empty "lines" (such as in the beginning of a file)
            // don't change the status of self.new_line.
            self.new_line = ends_with_newline;
        }
        Ok(())
    }

    fn output_conflict_marker(&mut self, s: &'a str) -> Result<()> {
        debug!("output_conflict_marker {:?}", self.new_line);
        if !self.new_line {
            self.write(s.as_bytes())?;
        } else {
            self.write(&s.as_bytes()[1..])?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Visits {
    pub first: usize,
    pub last: usize,
    /// Does this vertex have incomparable children?
    pub begins_conflict: Option<usize>,
    /// Does this vertex have incoming cross-edges?
    pub ends_conflict: bool,
    /// Has the line been output? (for the second DFS, in
    /// conflict_tree)
    pub output: bool,
}

impl Default for Visits {
    fn default() -> Self {
        Visits {
            first: 0,
            last: 0,
            begins_conflict: None,
            ends_conflict: false,
            output: false,
        }
    }
}

pub struct DFS {
    visits: Vec<Visits>,
    counter: usize,
    has_conflicts: bool,
}

impl DFS {
    pub fn new(n: usize) -> Self {
        DFS {
            visits: vec![Visits::default(); n],
            counter: 1,
            has_conflicts: false,
        }
    }
}

impl DFS {
    fn mark_discovered(&mut self, scc: usize) {
        if self.visits[scc].first == 0 {
            self.visits[scc].first = self.counter;
            self.counter += 1;
        }
    }

    fn mark_last_visit(&mut self, scc: usize) {
        self.mark_discovered(scc);
        self.visits[scc].last = self.counter;
        self.counter += 1;
    }

    fn first_visit(&self, scc: usize) -> usize {
        self.visits[scc].first
    }

    fn last_visit(&self, scc: usize) -> usize {
        self.visits[scc].last
    }
}

impl Graph {
    /// Tarjan's strongly connected component algorithm, returning a
    /// vector of strongly connected components, where each SCC is a
    /// vector of vertex indices.
    fn tarjan(&mut self) -> Vec<Vec<VertexId>> {
        if self.lines.len() <= 1 {
            return vec![vec![VertexId(0)]];
        }

        let mut call_stack = vec![(VertexId(1), 0, true)];

        let mut index = 0;
        let mut stack = Vec::new();
        let mut scc = Vec::new();
        while let Some((n_l, i, first_visit)) = call_stack.pop() {
            if first_visit {
                // First time we visit this node.
                let ref mut l = self[n_l];
                debug!("tarjan: {:?}", l.key);
                (*l).index = index;
                (*l).lowlink = index;
                (*l).flags = (*l).flags | Flags::LINE_ONSTACK | Flags::LINE_VISITED;
                debug!("{:?} {:?} chi", (*l).key, (*l).n_children);
                stack.push(n_l);
                index = index + 1;
            } else {
                let &(_, n_child) = self.child(n_l, i);
                self[n_l].lowlink = std::cmp::min(self[n_l].lowlink, self[n_child].lowlink);
            }

            let call_stack_length = call_stack.len();
            for j in i..self[n_l].n_children {
                let &(_, n_child) = self.child(n_l, j);
                if !self[n_child].flags.contains(Flags::LINE_VISITED) {
                    call_stack.push((n_l, j, false));
                    call_stack.push((n_child, 0, true));
                    break;
                // self.tarjan_dfs(scc, stack, index, n_child);
                } else {
                    if self[n_child].flags.contains(Flags::LINE_ONSTACK) {
                        self[n_l].lowlink = min(self[n_l].lowlink, self[n_child].index)
                    }
                }
            }
            if call_stack_length < call_stack.len() {
                // recursive call
                continue;
            }
            // Here, all children of n_l have been visited.

            if self[n_l].index == self[n_l].lowlink {
                let mut v = Vec::new();
                loop {
                    match stack.pop() {
                        None => break,
                        Some(n_p) => {
                            self[n_p].scc = scc.len();
                            self[n_p].flags = self[n_p].flags ^ Flags::LINE_ONSTACK;
                            v.push(n_p);
                            if n_p == n_l {
                                break;
                            }
                        }
                    }
                }
                scc.push(v);
            }
        }
        scc
    }

    /// Run a depth-first search on this graph, assigning the
    /// `first_visit` and `last_visit` numbers to each node.
    fn dfs<A: Transaction, R>(
        &mut self,
        txn: &GenericTxn<A, R>,
        branch: &Branch,
        scc: &[Vec<VertexId>],
        dfs: &mut DFS,
        forward: &mut Vec<(Key<PatchId>, Edge)>,
    ) {
        let mut call_stack: Vec<(_, HashSet<usize>, _)> = Vec::with_capacity(scc.len());
        call_stack.push((scc.len() - 1, HashSet::new(), None));

        let mut conflict_stack: Vec<(usize, usize)> = Vec::new();
        debug!("dfs starting");
        while let Some((n_scc, mut forward_scc, descendants)) = call_stack.pop() {
            debug!("dfs, n_scc = {:?}", n_scc);
            for &VertexId(id) in scc[n_scc].iter() {
                debug!("dfs, n_scc: {:?}", self.lines[id].key);
            }
            dfs.mark_discovered(n_scc);
            debug!(
                "scc: {:?}, first {} last {}",
                n_scc,
                dfs.first_visit(n_scc),
                dfs.last_visit(n_scc)
            );
            let is_first_visit = descendants.is_none();
            let mut descendants = if let Some(descendants) = descendants {
                descendants
            } else {
                // First visit / discovery of SCC n_scc.

                // After Tarjan's algorithm, the SCC numbers are in reverse
                // topological order.
                //
                // Here, we want to visit the first child in topological
                // order, hence the one with the largest SCC number first.
                //

                // Collect all descendants of this SCC, in order of increasing
                // SCC.
                let mut descendants = Vec::new();
                for cousin in scc[n_scc].iter() {
                    for &(e, n_child) in self.children(*cousin) {
                        let is_ok = match e {
                            Some(e) if e.flag.contains(EdgeFlags::FOLDER_EDGE) => false,
                            _ => true
                        };
                        if is_ok {
                            let child_component = self[n_child].scc;
                            if child_component < n_scc {
                                // If this is a child and not a sibling.
                                descendants.push(child_component)
                            }
                        }
                    }
                }
                descendants.sort();
                debug!("sorted descendants: {:?}", descendants);
                descendants
            };

            // SCCs to which we have forward edges.
            let mut recursive_call = None;
            while let Some(child) = descendants.pop() {
                debug!(
                    "child {:?}, first {} last {}",
                    child,
                    dfs.first_visit(child),
                    dfs.last_visit(child)
                );

                if dfs.first_visit(child) == 0 {
                    // SCC `child` has not yet been visited, visit it.

                    // If this is not our first visit to SCC `n_scc`,
                    // mark it as the beginning of a conflict.
                    if !is_first_visit {
                        debug!("{:?}: begins conflict", n_scc);
                        dfs.has_conflicts = true;
                        dfs.visits[n_scc].begins_conflict = Some(n_scc);
                        if let Some((scc, end)) = conflict_stack.pop() {
                            conflict_stack.push((scc, end));
                            if scc != n_scc {
                                conflict_stack.push((n_scc, n_scc));
                            }
                        } else {
                            conflict_stack.push((n_scc, n_scc));
                        }
                    }
                    recursive_call = Some(child);
                    break;
                } else if dfs.first_visit(n_scc) < dfs.first_visit(child) {
                    // This is a forward edge.
                    debug!("last_visit to {:?}: {:?}", child, dfs.last_visit(child));
                    forward_scc.insert(child);
                } else {
                    // cross edge
                    debug!(
                        "cross edge, stack: {:?}, updating with {:?}",
                        conflict_stack, child
                    );
                    for c in conflict_stack.iter_mut() {
                        c.1 = std::cmp::min(c.1, child);
                    }
                    dfs.visits[child].ends_conflict = true
                }
            }
            if let Some(child) = recursive_call {
                call_stack.push((n_scc, forward_scc, Some(descendants)));
                call_stack.push((child, HashSet::new(), None));
            } else {
                dfs.mark_last_visit(n_scc);
                if dfs.visits[n_scc].begins_conflict.is_some() {
                    let (begin, end) = conflict_stack.pop().unwrap();
                    assert_eq!(begin, n_scc);
                    debug!("begins_conflict {:?}: {:?}", n_scc, end);
                    dfs.visits[n_scc].begins_conflict = Some(end);
                }

                // After this, collect forward edges. Look at all
                // children of this SCC.
                for cousin in scc[n_scc].iter() {
                    for &(edge, n_child) in self.children(*cousin) {
                        if let Some(mut edge) = edge {
                            // Is this edge a forward edge of the DAG?
                            if forward_scc.contains(&self[n_child].scc)
                                && edge.flag.contains(EdgeFlags::PSEUDO_EDGE)
                                && !txn.test_edge(
                                    branch,
                                    self[*cousin].key,
                                    edge.dest,
                                    EdgeFlags::DELETED_EDGE,
                                    EdgeFlags::DELETED_EDGE,
                                ) {
                                debug!("forward: {:?} {:?}", self[*cousin].key, edge);
                                forward.push((self[*cousin].key, edge))
                            } else {
                                // Does this edge have parallel PSEUDO edges? If so, they may not be "forward", but they are redundant.
                                edge.flag = EdgeFlags::PSEUDO_EDGE;
                                edge.introduced_by = ROOT_PATCH_ID;
                                let mut edges = txn
                                    .iter_nodes(branch, Some((self[*cousin].key, Some(edge))))
                                    .take_while(|&(k, v)| {
                                        k == self[*cousin].key
                                            && v.dest == edge.dest
                                            && v.flag
                                                <= EdgeFlags::FOLDER_EDGE | EdgeFlags::PSEUDO_EDGE
                                    });
                                edges.next(); // ignore the first pseudo-edge.
                                forward.extend(edges.map(|(k, v)| (k, v))); // add all parallel edges.
                            }
                        }
                    }
                }
            }
        }
    }
}

impl<A: Transaction, R> GenericTxn<A, R> {
    /// This function constructs a graph by reading the branch from the
    /// input key. It guarantees that all nodes but the first one (index
    /// 0) have a common descendant, which is index 0.
    pub fn retrieve<'a>(&'a self, branch: &Branch, key0: Key<PatchId>) -> Graph {
        let mut graph = Graph {
            lines: Vec::new(),
            children: Vec::new(),
        };
        // Insert last "dummy" line (so that all lines have a common descendant).
        graph.lines.push(Line {
            key: ROOT_KEY,
            flags: Flags::empty(),
            children: 0,
            n_children: 0,
            index: 0,
            lowlink: 0,
            scc: 0,
        });

        // Avoid the root key.
        let mut cache: HashMap<Key<PatchId>, VertexId> = HashMap::new();
        cache.insert(ROOT_KEY.clone(), DUMMY_VERTEX);
        let mut stack = Vec::new();
        if self.get_nodes(&branch, key0, None).is_some() {
            stack.push(key0)
        }
        while let Some(key) = stack.pop() {
            if cache.contains_key(&key) {
                // We're doing a DFS here, this can definitely happen.
                continue;
            }

            let idx = VertexId(graph.lines.len());
            cache.insert(key.clone(), idx);

            debug!("{:?}", key);
            let mut is_zombie = false;
            // Does this vertex have a DELETED/DELETED+FOLDER edge
            // pointing to it?
            let mut first_edge = Edge::zero(EdgeFlags::PARENT_EDGE | EdgeFlags::DELETED_EDGE);
            let mut nodes = self.iter_nodes(&branch, Some((key, Some(first_edge))));
            if let Some((k, v)) = nodes.next() {
                debug!("zombie? {:?} {:?}", k, v);
                if k == key
                    && (v.flag | EdgeFlags::FOLDER_EDGE == first_edge.flag | EdgeFlags::FOLDER_EDGE)
                {
                    // Does this vertex also have an alive edge
                    // pointing to it? (might not be the case for the
                    // first vertex)
                    if key == key0 {
                        first_edge.flag = EdgeFlags::PARENT_EDGE;
                        nodes = self.iter_nodes(&branch, Some((key, Some(first_edge))));
                        if let Some((_, v)) = nodes.next() {
                            // We know the key is `key`.
                            is_zombie = v.flag | EdgeFlags::FOLDER_EDGE
                                == first_edge.flag | EdgeFlags::FOLDER_EDGE
                        }
                    } else {
                        is_zombie = true
                    }
                }
            }
            debug!("is_zombie: {:?}", is_zombie);
            let mut l = Line {
                key: key.clone(),
                flags: if is_zombie {
                    Flags::LINE_HALF_DELETED
                } else {
                    Flags::empty()
                },
                children: graph.children.len(),
                n_children: 0,
                index: 0,
                lowlink: 0,
                scc: 0,
            };

            let mut last_flag = EdgeFlags::empty();
            let mut last_dest = ROOT_KEY;

            for (_, v) in self
                .iter_nodes(&branch, Some((key, None)))
                .take_while(|&(k, v)| {
                    k == key
                        && v.flag
                            <= EdgeFlags::PSEUDO_EDGE
                                | EdgeFlags::FOLDER_EDGE
                                | EdgeFlags::EPSILON_EDGE
                }) {
                debug!("-> v = {:?}", v);
                if last_flag == EdgeFlags::PSEUDO_EDGE && last_dest == v.dest {
                    // This is a doubled edge, it should be removed.
                } else {
                    graph.children.push((Some(v.clone()), DUMMY_VERTEX));
                    l.n_children += 1;
                    if !cache.contains_key(&v.dest) {
                        stack.push(v.dest.clone())
                    } else {
                        debug!("v already visited");
                    }
                    last_flag = v.flag;
                    last_dest = v.dest;
                }
            }
            // If this key has no children, give it the dummy child.
            if l.n_children == 0 {
                debug!("no children for {:?}", l);
                graph.children.push((None, DUMMY_VERTEX));
                l.n_children = 1;
            }
            graph.lines.push(l)
        }
        for &mut (ref child_key, ref mut child_idx) in graph.children.iter_mut() {
            if let Some(ref child_key) = *child_key {
                if let Some(idx) = cache.get(&child_key.dest) {
                    *child_idx = *idx
                }
            }
        }
        graph
    }
}

/// The conflict markers keep track of the number of conflicts, and is
/// used for outputting conflicts to a given LineBuffer.
///
/// "Zombie" conflicts are those conflicts introduced by zombie
/// vertices in the contained Graph.
struct ConflictMarkers<'b> {
    current_is_zombie: bool,
    current_conflicts: usize,
    graph: &'b Graph,
}

impl<'b> ConflictMarkers<'b> {
    fn output_zombie_markers_if_needed<'a, A: Transaction + 'a, B: LineBuffer<'a, A>>(
        &mut self,
        buf: &mut B,
        vertex: VertexId,
    ) -> Result<()> {
        if self.graph[vertex].is_zombie() {
            if !self.current_is_zombie {
                debug!("begin zombie conflict: vertex = {:?}", self.graph[vertex]);
                self.current_is_zombie = true;
                buf.begin_conflict()?;
            }
        } else if self.current_is_zombie {
            // Zombie segment has ended
            if !self.current_is_zombie {
                debug!("end zombie conflict: vertex = {:?}", self.graph[vertex]);
            }
            self.current_is_zombie = false;
            buf.end_conflict()?;
        }
        Ok(())
    }

    fn begin_conflict<'a, A: Transaction + 'a, B: LineBuffer<'a, A>>(
        &mut self,
        buf: &mut B,
    ) -> Result<()> {
        buf.begin_conflict()?;
        self.current_conflicts += 1;
        Ok(())
    }
    fn end_conflict<'a, A: Transaction + 'a, B: LineBuffer<'a, A>>(
        &mut self,
        buf: &mut B,
    ) -> Result<()> {
        if self.current_conflicts > 0 {
            buf.end_conflict()?;
            self.current_conflicts -= 1;
        }
        Ok(())
    }
}

/// In the case of nested conflicts, a single conflict sometimes needs
/// to be treated like a line.
#[derive(Debug, Clone)]
enum ConflictLine {
    Conflict(Vec<Vec<ConflictLine>>),
    Line(usize),
}

#[derive(Debug)]
struct Side {
    next: usize,
    side: Vec<ConflictLine>,
}

#[derive(Debug)]
enum State {
    Init {
        resume_conflict: bool,
    },
    EvalConflict {
        start: usize,
        end: usize,
        cur: usize,
        last_visit: usize,
        sides: Vec<Side>,
    },
}

impl<'a, A: Transaction + 'a, R> GenericTxn<A, R> {
    fn output_conflict<B: LineBuffer<'a, A>>(
        &'a self,
        conflicts: &mut ConflictMarkers,
        buf: &mut B,
        graph: &Graph,
        scc: &[Vec<VertexId>],
        conflict: &mut [Vec<ConflictLine>],
    ) -> Result<()> {
        let mut is_first = true;
        let n_sides = conflict.len();
        debug!(target:"libpijul::graph::output_conflict", "n_sides = {:?}", n_sides);
        if n_sides > 1 {
            conflicts.begin_conflict(buf)?;
        }
        for side in conflict {
            if !is_first {
                buf.conflict_next()?;
            }
            is_first = false;
            debug!(target:"libpijul::graph::output_conflict", "side = {:?}", side);
            for i in side {
                match *i {
                    ConflictLine::Line(i) => self.output_scc(conflicts, graph, &scc[i], buf)?,
                    ConflictLine::Conflict(ref mut c) => {
                        debug!(target:"libpijul::graph::output_conflict", "begin conflict {:?}", c);
                        self.output_conflict(conflicts, buf, graph, scc, c)?;
                        debug!(target:"libpijul::graph::output_conflict", "end conflict");
                    }
                }
            }
        }
        if n_sides > 1 {
            conflicts.end_conflict(buf)?;
        }
        Ok(())
    }

    /// Output the database contents of the file into the buffer
    /// `buf`. The return value indicates whether there are any
    /// conflicts in the file that was output. If forward edges are
    /// encountered, they are collected into `forward`.
    ///
    pub fn output_file<B: LineBuffer<'a, A>>(
        &'a self,
        branch: &Branch,
        buf: &mut B,
        graph: &mut Graph,
        forward: &mut Vec<(Key<PatchId>, Edge)>,
    ) -> Result<bool> {
        debug!("output_file");
        if graph.lines.len() <= 1 {
            return Ok(false);
        }
        let scc = graph.tarjan(); // SCCs are given here in reverse order.
        debug!("There are {} SCC", scc.len());
        debug!("SCCs = {:?}", scc);

        let mut dfs = DFS::new(scc.len());
        graph.dfs(self, branch, &scc, &mut dfs, forward);

        debug!("dfs done");
        buf.output_line(&graph.lines[1].key, Value::from_slice(b""))?;
        let conflict_tree = conflict_tree(graph, &scc, &mut dfs);
        debug!("conflict_tree = {:?}", conflict_tree);
        let mut conflicts = ConflictMarkers {
            current_is_zombie: false,
            current_conflicts: 0,
            graph: &graph,
        };
        self.output_conflict(&mut conflicts, buf, graph, &scc, &mut [conflict_tree])?;
        // Close any remaining zombie part (if needed).
        conflicts.output_zombie_markers_if_needed(buf, DUMMY_VERTEX)?;
        debug!("/output_file");
        Ok(dfs.has_conflicts)
    }

    fn output_scc<B: LineBuffer<'a, A>>(
        &'a self,
        conflicts: &mut ConflictMarkers,
        graph: &Graph,
        scc: &[VertexId],
        buf: &mut B,
    ) -> Result<()> {
        assert_eq!(scc.len(), 1);
        conflicts.output_zombie_markers_if_needed(buf, scc[0])?;
        let key = graph[scc[0]].key;
        if let Some(cont) = self.get_contents(key) {
            debug!(target:"libpijul::graph::output_conflict", "outputting {:?}", cont);
            buf.output_line(&key, cont)?;
        }
        Ok(())
    }
}

fn conflict_tree(graph: &Graph, scc: &[Vec<VertexId>], dfs: &mut DFS) -> Vec<ConflictLine> {
    debug!(target: "tree", "scc = {:?}", scc);
    let mut call_stack = Vec::with_capacity(scc.len());
    call_stack.push((
        scc.len() - 1,
        State::EvalConflict {
            start: scc.len() - 1,
            end: 0,
            cur: 1,
            last_visit: 0,
            sides: vec![Side {
                next: scc.len() - 1,
                side: vec![],
            }],
        },
    ));
    debug!(target: "tree", "conflict tree starting");

    while let Some((i, state)) = call_stack.pop() {
        debug!(target: "tree", "{:?} ({:?}): {:?}, {:?}", i, scc[i], graph[scc[i][0]].key, dfs.visits[i]);
        debug!(target: "tree", "{:?} {:?}", call_stack, state);
        match state {
            State::Init { resume_conflict } => {
                debug!(target:"tree", "new visit to {:?}", graph[scc[i][0]].key);
                dfs.visits[i].output = true;
                if dfs.visits[i].ends_conflict && !resume_conflict {
                    debug!(target: "tree", "ends conflict");
                    if let Some((
                        _,
                        State::EvalConflict {
                            ref mut sides,
                            ref mut cur,
                            ..
                        },
                    )) = call_stack.last_mut()
                    {
                        sides[*cur].next = i
                    }
                } else {
                    if let Some((
                        _,
                        State::EvalConflict {
                            ref cur,
                            ref mut sides,
                            ..
                        },
                    )) = call_stack.last_mut()
                    {
                        sides[*cur].side.push(ConflictLine::Line(i))
                    }
                    if let Some(c) = dfs.visits[i].begins_conflict {
                        let mut sides = Vec::new();
                        for cousin in scc[i].iter() {
                            for &(e, n_child) in graph.children(*cousin) {
                                let is_ok = match e {
                                    Some(e) if e.flag.contains(EdgeFlags::FOLDER_EDGE) => false,
                                    _ => true
                                };
                                if is_ok {
                                    let next = graph[n_child].scc;
                                    sides.push(Side {
                                        next,
                                        side: Vec::new(),
                                    })
                                }
                            }
                        }
                        sides.sort_by(|a, b| a.next.cmp(&b.next));
                        debug!(target: "tree", "sides: {:?}", sides);
                        let cur = sides.len();
                        call_stack.push((
                            i,
                            State::EvalConflict {
                                sides,
                                start: i,
                                end: c,
                                last_visit: 0,
                                cur,
                            },
                        ));
                    } else {
                        let mut max_scc = None;
                        for cousin in scc[i].iter() {
                            for &(e, n_child) in graph.children(*cousin) {
                                let is_ok = match e {
                                    Some(e) if e.flag.contains(EdgeFlags::FOLDER_EDGE) => false,
                                    _ => true
                                };
                                if is_ok {
                                    let next = graph[n_child].scc;
                                    max_scc = std::cmp::max(max_scc, Some(next))
                                }
                            }
                        }
                        if let Some(max_scc) = max_scc {
                            call_stack.push((
                                max_scc,
                                State::Init {
                                    resume_conflict: false,
                                },
                            ))
                        }
                    }
                }
            }
            State::EvalConflict {
                start,
                end,
                mut cur,
                last_visit,
                mut sides,
            } => {
                if cur > 0 {
                    // Recurse
                    let next = sides[cur - 1].next;
                    let visit_next = dfs.visits[next].first > last_visit;
                    call_stack.push((
                        i,
                        State::EvalConflict {
                            start,
                            end,
                            cur: cur - 1,
                            last_visit: if visit_next {
                                dfs.visits[next].last
                            } else {
                                last_visit
                            },
                            sides,
                        },
                    ));
                    if visit_next {
                        call_stack.push((
                            next,
                            State::Init {
                                resume_conflict: dfs.visits[next].ends_conflict,
                            },
                        ));
                    }
                } else if call_stack.is_empty() {
                    // We're done, return.
                    assert_eq!(sides.len(), 1);
                    let side = sides.pop().unwrap();
                    assert!(sides.is_empty());
                    return side.side;
                } else {
                    sides.sort_by(|a, b| a.next.cmp(&b.next));
                    let conflict_is_over = {
                        let next = sides.iter().filter(|x| !x.side.is_empty()).next().unwrap().next;
                        sides.iter().all(|x| x.side.is_empty() || x.next == next)
                    };
                    if conflict_is_over {
                        // The conflict is over.
                        let current = sides
                            .into_iter()
                            .filter_map(|x| {
                                if x.side.is_empty() {
                                    None
                                } else {
                                    Some(x.side)
                                }
                            }).collect();
                        if let Some((
                            _,
                            State::EvalConflict {
                                ref cur,
                                ref mut sides,
                                ..
                            },
                        )) = call_stack.last_mut()
                        {
                            sides[*cur].side.push(ConflictLine::Conflict(current));
                        }
                        call_stack.push((
                            end,
                            State::Init {
                                resume_conflict: true,
                            },
                        ));
                    } else {
                        let mut sides_ = Vec::new();
                        let mut next = sides[0].next;
                        let mut current = Vec::new();
                        debug!(target:"tree", "sides {:?}", sides);
                        for side in sides.into_iter().filter(|x| !x.side.is_empty()) {
                            debug!(target:"tree", "side {:?}", side);
                            if side.next != next {
                                let cur = std::mem::replace(&mut current, Vec::new());
                                if !cur.is_empty() {
                                    sides_.push(Side {
                                        next,
                                        side: vec![ConflictLine::Conflict(cur)],
                                    })
                                }
                                next = side.next;
                            }
                            current.push(side.side);
                        }
                        if !current.is_empty() {
                            sides_.push(Side {
                                next,
                                side: vec![ConflictLine::Conflict(current)],
                            });
                        }
                        debug!(target:"tree", "conflict reduction {:?}", sides_);
                        assert!(sides_.len() > 1);
                        call_stack.push((
                            i,
                            State::EvalConflict {
                                start,
                                end,
                                cur: sides_.len(),
                                last_visit: 0,
                                sides: sides_,
                            },
                        ));
                    }
                }
            }
        }
    }
    unreachable!()
}

/// Removes redundant forward edges, among those listed in `forward`.
impl<'env, R: rand::Rng> MutTxn<'env, R> {
    pub fn remove_redundant_edges(
        &mut self,
        branch: &mut Branch,
        forward: &[(Key<PatchId>, Edge)],
    ) -> Result<()> {
        for &(key, edge) in forward.iter() {
            self.del_edge_both_dirs(branch, key, edge)?;
        }
        Ok(())
    }
}
