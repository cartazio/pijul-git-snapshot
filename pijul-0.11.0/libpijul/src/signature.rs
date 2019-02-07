//! Signing patches and verifying patch signatures.
use Error;
use serde::{Serialize, Deserialize, Deserializer, Serializer};
use serde::de::{Visitor, SeqAccess};
use serde::ser::SerializeTuple;
use serde;
use std::fmt;
use ring;
use untrusted;
use bs58;

/// Wrapper around `[u8; 64]`, mostly to implement serde traits.
pub struct SignatureBytes(pub [u8;64]);

/// A public and secret key.
pub enum KeyPair {
    Ed25519(ring::signature::Ed25519KeyPair)
}

impl KeyPair {

    /// Parse a `KeyPair` from a PKCS#8 binary buffer.
    pub fn from_pkcs8(input: &[u8]) -> Result<Self, Error> {
        Ok(KeyPair::Ed25519(
            ring::signature::Ed25519KeyPair::from_pkcs8(
                untrusted::Input::from(input)
            )?
        ))
    }

    /// Sign a message.
    pub fn sign(&self, msg: &[u8]) -> Signature {
        match *self {
            KeyPair::Ed25519(ref k) => {
                let mut pk = [0; 32];
                pk.clone_from_slice(k.public_key_bytes());
                let signature = k.sign(msg);
                let mut sig = SignatureBytes([0; 64]);
                sig.0.clone_from_slice(signature.as_ref());
                Signature::Ed25519 { publickey: pk, signature: sig }
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum Signature {
    Ed25519 { publickey: [u8; 32], signature: SignatureBytes }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PublicKey {
    Ed25519([u8; 32])
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReadableSignature {
    #[serde(rename="type")]
    typ_: String,
    #[serde(rename="publicKey")]
    public_key: String,
    signature: String,
}

impl Signature {

    pub fn to_readable(&self) -> ReadableSignature {
        match *self {
            Signature::Ed25519 { ref publickey, ref signature } => {
                ReadableSignature {
                    typ_: "ed25519".to_string(),
                    public_key: bs58::encode(publickey).into_string(),
                    signature: bs58::encode(&signature.0[..]).into_string(),
                }
            }
        }
    }

    pub fn key_type(&self) -> &'static str {
        match *self {
            Signature::Ed25519 { .. } => "ed25519",
        }
    }

    /// Public key corresponding to the secret key used to sign.
    pub fn public_key(&self) -> &[u8] {
        match *self {
            Signature::Ed25519 { ref publickey, .. } => publickey
        }
    }

    /// Verify the signature for the given message.
    pub fn verify<S:AsRef<[u8]>>(&self, msg: S) -> Result<(), Error> {
        match *self {
            Signature::Ed25519 { ref publickey, ref signature } => {
                Ok(ring::signature::verify(
                    &ring::signature::EdDSAParameters,
                    untrusted::Input::from(publickey),
                    untrusted::Input::from(msg.as_ref()),
                    untrusted::Input::from(&signature.0),
                )?)
            }
        }
    }

    /// Length of that signature in bytes, once serialized to bincode.
    pub fn len(&self) -> usize {
        match *self {
            Signature::Ed25519 { .. } =>
            // u32 (signature variant id) + 32 bytes + 64 bytes
                100
        }
    }
}

impl AsRef<[u8]> for Signature {
    fn as_ref(&self) -> &[u8] {
        match *self {
            Signature::Ed25519 { ref signature, .. } => &signature.0
        }
    }
}


impl<'de> Deserialize<'de> for SignatureBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        struct Vis;
        impl<'de> Visitor<'de> for Vis {
            type Value = SignatureBytes;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("64 bytes")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut result = [0; 64];
                for x in result.iter_mut() {
                    if let Some(y) = seq.next_element()? {
                        *x = y
                    } else {
                        return Err(serde::de::Error::invalid_length(64, &self))
                    }
                }
                Ok(SignatureBytes(result))
            }
        }
        deserializer.deserialize_tuple(64, Vis)
    }
}

impl Serialize for SignatureBytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut tup = serializer.serialize_tuple(64)?;
        for byte in self.0.iter() {
            tup.serialize_element(byte)?;
        }
        tup.end()
    }
}

impl fmt::Debug for SignatureBytes {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:?}", &self.0[..])
    }
}

impl Clone for SignatureBytes {
    fn clone(&self) -> Self {
        let mut result = SignatureBytes([0;64]);
        result.0.clone_from_slice(&self.0);
        result
    }
}
