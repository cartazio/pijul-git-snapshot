use bs58;
use byteorder::{ByteOrder, LittleEndian};
use sanakirja::{Alignment, Representable};
use serde;
use serde::de::{Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};
use std;
use {Error, LineId};

const SHA512_BYTES: usize = 512 / 8;

/// The external hash of patches.
#[derive(Serialize, Deserialize, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub enum Hash {
    /// None is the hash of the "null patch", which introduced a
    /// single root vertex at the beginning of the repository.
    None,
    /// Patch hashed using the SHA2-512 algorithm.
    Sha512(Sha512),
    /// Recursive patch hashes (patches written as text in the repository).
    Recursive(Recursive),
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct Recursive(Vec<u8>);

impl Recursive {
    pub fn patch(&self) -> HashRef {
        match unsafe { std::mem::transmute(self.0[0]) } {
            Algorithm::None => HashRef::None,
            Algorithm::Sha512 => HashRef::Sha512(&self.0[1..self.0.len() - 8]),
            Algorithm::Recursive => HashRef::Recursive(&self.0[1..self.0.len() - 8]),
        }
    }
    pub fn line(&self) -> LineId {
        match unsafe { std::mem::transmute(self.0[0]) } {
            Algorithm::None => LineId(0),
            Algorithm::Sha512 => LineId(LittleEndian::read_u64(&self.0[self.0.len() - 8..])),
            Algorithm::Recursive => LineId(LittleEndian::read_u64(&self.0[self.0.len() - 8..])),
        }
    }
}

impl Hash {
    pub fn recursive(&self, line: LineId) -> Self {
        match *self {
            Hash::None => Hash::Recursive(Recursive(vec![Algorithm::None as u8])),
            Hash::Sha512(ref p) => {
                let mut v = Vec::new();
                v.push(Algorithm::Sha512 as u8);
                v.extend(&p.0[..]);
                v.extend(b"\0\0\0\0\0\0\0\0");
                LittleEndian::write_u64(&mut v[SHA512_BYTES..], line.0);
                Hash::Recursive(Recursive(v))
            }
            Hash::Recursive(Recursive(ref p)) => {
                let mut v = Vec::new();
                v.push(Algorithm::Recursive as u8);
                v.extend(p);
                v.extend(b"\0\0\0\0\0\0\0\0");
                LittleEndian::write_u64(&mut v[p.len()..], line.0);
                Hash::Recursive(Recursive(v))
            }
        }
    }
}

pub struct Sha512(pub [u8; SHA512_BYTES]);

impl PartialEq for Sha512 {
    fn eq(&self, h: &Sha512) -> bool {
        (&self.0[..]).eq(&h.0[..])
    }
}
impl Eq for Sha512 {}
impl PartialOrd for Sha512 {
    fn partial_cmp(&self, h: &Sha512) -> Option<std::cmp::Ordering> {
        (&self.0[..]).partial_cmp(&h.0[..])
    }
}
impl Ord for Sha512 {
    fn cmp(&self, h: &Sha512) -> std::cmp::Ordering {
        (&self.0[..]).cmp(&h.0[..])
    }
}

impl std::hash::Hash for Sha512 {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        (&self.0[..]).hash(h)
    }
}

struct Sha512Visitor;
impl<'a> Visitor<'a> for Sha512Visitor {
    type Value = Sha512;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "A byte slice of length {}", SHA512_BYTES)
    }

    fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
        let mut x: [u8; SHA512_BYTES] = [0; SHA512_BYTES];
        x.copy_from_slice(v);
        Ok(Sha512(x))
    }
}

impl<'a> Deserialize<'a> for Sha512 {
    fn deserialize<D: Deserializer<'a>>(d: D) -> Result<Sha512, D::Error> {
        d.deserialize_bytes(Sha512Visitor)
    }
}

impl Serialize for Sha512 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(&self.0[..])
    }
}

impl std::fmt::Debug for Sha512 {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        (&self.0[..]).fmt(fmt)
    }
}
impl<'a> std::fmt::Debug for HashRef<'a> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{}", self.to_base58())
    }
}
impl std::fmt::Debug for Hash {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.as_ref().fmt(fmt)
    }
}

/// A borrowed version of `Hash`.
#[derive(Copy, Clone, Hash, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum HashRef<'a> {
    None,
    Sha512(&'a [u8]),
    Recursive(&'a [u8]),
}

impl Hash {
    /// Get a `Hash` from a binary slice. This function does not
    /// compute the digest of anything, it just converts types.
    pub fn from_binary(v: &[u8]) -> Option<Self> {
        if v.len() == 0 {
            None
        } else {
            if v[0] == Algorithm::Sha512 as u8 && v.len() == 1 + SHA512_BYTES {
                let mut hash = [0; SHA512_BYTES];
                hash.clone_from_slice(&v[1..]);
                Some(Hash::Sha512(Sha512(hash)))
            } else if v[0] == Algorithm::None as u8 && v.len() == 1 {
                Some(Hash::None)
            } else {
                None
            }
        }
    }

    /// Decode a hash from a base58-encoded `str`.
    pub fn from_base58(base58: &str) -> Option<Self> {
        if let Ok(v) = bs58::decode(base58).into_vec() {
            Self::from_binary(&v)
        } else {
            None
        }
    }

    /// A borrowed version of this `Hash`, used for instance to
    /// query the databases.
    pub fn as_hash_ref(&self) -> HashRef {
        match *self {
            Hash::None => HashRef::None,
            Hash::Sha512(ref e) => HashRef::Sha512(unsafe {
                std::slice::from_raw_parts(e.0.as_ptr() as *const u8, SHA512_BYTES)
            }),
            Hash::Recursive(Recursive(ref p)) => HashRef::Recursive(p.as_slice()),
        }
    }

    pub fn as_ref(&self) -> HashRef {
        self.as_hash_ref()
    }

    /// Create a `Hash` from the binary slice of the patch contents.
    pub fn of_slice(buf: &[u8]) -> Result<Hash, Error> {
        use openssl::hash::*;
        let hash = {
            let mut hasher = Hasher::new(MessageDigest::sha512())?;
            hasher.update(buf)?;
            hasher.finish()?
        };
        let mut digest: [u8; SHA512_BYTES] = [0; SHA512_BYTES];
        digest.clone_from_slice(hash.as_ref());
        Ok(Hash::Sha512(Sha512(digest)))
    }
}

impl<'a> HashRef<'a> {
    /// Encode this `HashRef` in binary.
    pub fn to_binary(&self) -> Vec<u8> {
        let u = self.to_unsafe();
        let mut v = vec![0; u.onpage_size() as usize];
        unsafe { u.write_value(v.as_mut_ptr()) }
        v
    }

    /// Encode this `HashRef` in base58.
    pub fn to_base58(&self) -> String {
        bs58::encode(&self.to_binary()).into_string()
    }
}
impl Hash {
    /// Encode this `Hash` in base64.
    pub fn to_base58(&self) -> String {
        self.as_ref().to_base58()
    }
}

impl<'a> HashRef<'a> {
    /// Build an owned version of a `HashRef`.
    pub fn to_owned(&self) -> Hash {
        match *self {
            HashRef::None => Hash::None,
            HashRef::Sha512(e) => {
                let mut hash = [0; SHA512_BYTES];
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        e.as_ptr() as *const u8,
                        hash.as_mut_ptr() as *mut u8,
                        SHA512_BYTES,
                    )
                }
                Hash::Sha512(Sha512(hash))
            }
            HashRef::Recursive(p) => Hash::Recursive(Recursive(p.to_vec())),
        }
    }
}

impl Clone for Hash {
    fn clone(&self) -> Self {
        self.as_ref().to_owned()
    }
}

pub const ROOT_HASH: &'static Hash = &Hash::None;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u8)]
pub enum Algorithm {
    None = 0,
    Sha512 = 1,
    Recursive = 2,
}

#[derive(Clone, Copy, Debug)]
pub enum UnsafeHash {
    None,
    Sha512(*const u8),
    Recursive { ptr: *const u8, len: usize },
}

impl<'a> HashRef<'a> {
    pub fn to_unsafe(&self) -> UnsafeHash {
        match *self {
            HashRef::None => UnsafeHash::None,
            HashRef::Sha512(e) => UnsafeHash::Sha512(e.as_ptr()),
            HashRef::Recursive(p) => UnsafeHash::Recursive {
                ptr: p.as_ptr(),
                len: p.len(),
            },
        }
    }
    pub unsafe fn from_unsafe(p: UnsafeHash) -> HashRef<'a> {
        match p {
            UnsafeHash::None => HashRef::None,
            UnsafeHash::Sha512(p) => HashRef::Sha512(std::slice::from_raw_parts(p, SHA512_BYTES)),
            UnsafeHash::Recursive { ptr, len } => {
                HashRef::Recursive(std::slice::from_raw_parts(ptr, len))
            }
        }
    }
}

impl Representable for UnsafeHash {
    fn alignment() -> Alignment {
        Alignment::B1
    }

    fn onpage_size(&self) -> u16 {
        1 +
            (match *self {
                 UnsafeHash::Sha512(_) => 64,
                 UnsafeHash::None => 0,
                 UnsafeHash::Recursive { len, .. } => len as u16,
             })
    }
    unsafe fn write_value(&self, p: *mut u8) {
        trace!("write_value {:?} {:?}", self, p);
        match *self {
            UnsafeHash::Sha512(q) => {
                *p = Algorithm::Sha512 as u8;
                std::ptr::copy(q, p.offset(1), 64)
            }
            UnsafeHash::None => *p = Algorithm::None as u8,
            UnsafeHash::Recursive { ptr, len } => {
                *p = Algorithm::Recursive as u8;
                std::ptr::copy(ptr, p.offset(1), len)
            }
        }
    }
    unsafe fn read_value(p: *const u8) -> Self {
        trace!("read_value {:?} {:?}", p, *p);
        match std::mem::transmute(*p) {
            Algorithm::Sha512 => UnsafeHash::Sha512(p.offset(1)),
            Algorithm::None => UnsafeHash::None,
            Algorithm::Recursive => {
                let rec = Self::read_value(p.offset(1));
                UnsafeHash::Recursive {
                    ptr: p.offset(1),
                    len: rec.onpage_size() as usize,
                }
            }
        }
    }
    unsafe fn cmp_value<T>(&self, _: &T, x: Self) -> std::cmp::Ordering {
        let a = HashRef::from_unsafe(*self);
        let b = HashRef::from_unsafe(x);
        a.cmp(&b)
    }
    type PageOffsets = std::iter::Empty<u64>;
    fn page_offsets(&self) -> Self::PageOffsets {
        std::iter::empty()
    }
}
