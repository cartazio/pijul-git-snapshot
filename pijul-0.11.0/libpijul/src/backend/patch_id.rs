use bs58;
use byteorder::{ByteOrder, LittleEndian};
use sanakirja::{Alignment, Representable};
use std;

// Patch Identifiers.
pub const PATCH_ID_SIZE: usize = 8;
pub const ROOT_PATCH_ID: PatchId = PatchId(0);

/// An internal patch identifier, less random than external patch
/// identifiers, but more stable in time, and much smaller.
#[derive(Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Hash, Serialize, Deserialize)]
pub struct PatchId(pub(crate) u64);

impl std::fmt::Debug for PatchId {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "PatchId({})", self.to_base58())
    }
}

use hex::ToHex;
use std::fmt::Write;

impl ToHex for PatchId {
    fn write_hex<W: Write>(&self, w: &mut W) -> std::fmt::Result {
        let mut x = [0; 8];
        LittleEndian::write_u64(&mut x, self.0);
        x.write_hex(w)
    }

    fn write_hex_upper<W: Write>(&self, w: &mut W) -> std::fmt::Result {
        let mut x = [0; 8];
        LittleEndian::write_u64(&mut x, self.0);
        x.write_hex_upper(w)
    }
}

impl PatchId {
    /// New patch id (initialised to 0).
    pub fn new() -> Self {
        PatchId(0)
    }

    pub fn from_slice(s: &[u8]) -> Self {
        PatchId(LittleEndian::read_u64(s))
    }

    /// Encode this patch id in base58.
    pub fn to_base58(&self) -> String {
        let mut x = [0; 8];
        LittleEndian::write_u64(&mut x, self.0);
        bs58::encode(&x).into_string()
    }
    /// Decode this patch id from its base58 encoding.
    pub fn from_base58(s: &str) -> Option<Self> {
        let mut p = [0; 8];
        if bs58::decode(s).into(&mut p).is_ok() {
            Some(PatchId(LittleEndian::read_u64(&p)))
        } else {
            None
        }
    }
    pub fn is_root(&self) -> bool {
        *self == ROOT_PATCH_ID
    }
}

impl std::ops::Deref for PatchId {
    type Target = u64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for PatchId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Representable for PatchId {
    fn alignment() -> Alignment {
        Alignment::B8
    }
    fn onpage_size(&self) -> u16 {
        8
    }
    unsafe fn write_value(&self, p: *mut u8) {
        LittleEndian::write_u64(std::slice::from_raw_parts_mut(p, 8), self.0)
    }
    unsafe fn read_value(p: *const u8) -> Self {
        let x = PatchId(LittleEndian::read_u64(std::slice::from_raw_parts(p, 8)));
        trace!("read_value {:?} {:?}", p, x.to_base58());
        x
    }
    unsafe fn cmp_value<T>(&self, _: &T, x: Self) -> std::cmp::Ordering {
        self.0.cmp(&x.0)
    }
    type PageOffsets = std::iter::Empty<u64>;
    fn page_offsets(&self) -> Self::PageOffsets {
        std::iter::empty()
    }
}
