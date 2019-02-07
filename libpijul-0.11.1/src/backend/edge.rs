use super::key::*;
use super::patch_id::*;
use sanakirja::*;
use std;
use byteorder::{LittleEndian, ByteOrder};

bitflags! {
    /// Possible flags of edges.
    ///
    /// Possible values are `PSEUDO_EDGE`, `FOLDER_EDGE`,
    /// `PARENT_EDGE` and `DELETED_EDGE`.
    #[derive(Serialize, Deserialize)]
    pub struct EdgeFlags: u8 {
        /// A pseudo-edge, computed when applying the patch to
        /// restore connectivity, and/or mark conflicts.
        const PSEUDO_EDGE = 1;
        /// An edge encoding file system hierarchy.
        const FOLDER_EDGE = 2;
        /// An epsilon-edge, i.e. a "non-transitive" edge used to
        /// solve conflicts.
        const EPSILON_EDGE = 4;
        /// A "reverse" edge (all edges in the graph have a reverse edge).
        const PARENT_EDGE = 8;
        /// An edge whose target (if not also `PARENT_EDGE`) or
        /// source (if also `PARENT_EDGE`) is marked as deleted.
        const DELETED_EDGE = 16;
    }
}

/// The target half of an edge in the repository graph.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct Edge {
    /// Flags of this edge.
    pub flag: EdgeFlags,
    /// Target of this edge.
    pub dest: Key<PatchId>,
    /// Patch that introduced this edge (possibly as a
    /// pseudo-edge, i.e. not explicitly in the patch, but
    /// computed from it).
    pub introduced_by: PatchId,
}
impl Edge {
    /// Create an edge with the flags set to `flags`, and other
    /// parameters to 0.
    pub fn zero(flag: EdgeFlags) -> Edge {
        Edge {
            flag: flag,
            dest: ROOT_KEY.clone(),
            introduced_by: ROOT_PATCH_ID.clone(),
        }
    }
}

impl Representable for Edge {
    fn alignment() -> Alignment {
        Alignment::B1
    }
    fn onpage_size(&self) -> u16 {
        std::mem::size_of::<Edge>() as u16
    }
    unsafe fn write_value(&self, p: *mut u8) {
        trace!("write_value {:?}", p);
        let s = std::slice::from_raw_parts_mut(p, 25);
        s[0] = (*self).flag.bits();
        LittleEndian::write_u64(&mut s[1..], (*self).dest.patch.0);
        LittleEndian::write_u64(&mut s[9..], (*self).dest.line.0);
        LittleEndian::write_u64(&mut s[17..], (*self).introduced_by.0);
    }
    unsafe fn read_value(p: *const u8) -> Self {
        trace!("read_value {:?}", p);
        let s = std::slice::from_raw_parts(p, 25);
        Edge {
            flag: EdgeFlags::from_bits(s[0]).unwrap(),
            dest: Key {
                patch: PatchId(LittleEndian::read_u64(&s[1..])),
                line: LineId(LittleEndian::read_u64(&s[9..])),
            },
            introduced_by: PatchId(LittleEndian::read_u64(&s[17..])),
        }
    }
    unsafe fn cmp_value<T>(&self, _: &T, x: Self) -> std::cmp::Ordering {
        let a: &Edge = self;
        let b: &Edge = &x;
        a.cmp(b)
    }
    type PageOffsets = std::iter::Empty<u64>;
    fn page_offsets(&self) -> Self::PageOffsets {
        std::iter::empty()
    }
}
