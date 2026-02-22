//! Rivest–Shamir–Adleman (RSA) private keys.

use crate::{public::RsaPublicKey, Error, Mpint, Result};
use core::fmt;
use encoding::{CheckedSum, Decode, Encode, Reader, Writer};
use subtle::{Choice, ConstantTimeEq};
use zeroize::Zeroize;

#[cfg(feature = "rsa")]
use {
    rand_core::CryptoRngCore,
    rsa::sha2::digest::const_oid::AssociatedOid,
    rsa::{pkcs1v15, traits::PrivateKeyParts},
};

/// RSA private key.
#[derive(Clone)]
pub struct RsaPrivateKey {
    /// RSA private exponent.
    pub d: Mpint,

    /// CRT coefficient: `(inverse of q) mod p`.
    pub iqmp: Mpint,

    /// First prime factor of `n`.
    pub p: Mpint,

    /// Second prime factor of `n`.
    pub q: Mpint,
}

impl ConstantTimeEq for RsaPrivateKey {
    fn ct_eq(&self, other: &Self) -> Choice {
        self.d.ct_eq(&other.d)
            & self.iqmp.ct_eq(&self.iqmp)
            & self.p.ct_eq(&other.p)
            & self.q.ct_eq(&other.q)
    }
}

impl Eq for RsaPrivateKey {}

impl PartialEq for RsaPrivateKey {
    fn eq(&self, other: &Self) -> bool {
        self.ct_eq(other).into()
    }
}

impl Decode for RsaPrivateKey {
    type Error = Error;

    fn decode(reader: &mut impl Reader) -> Result<Self> {
        let d = Mpint::decode(reader)?;
        let iqmp = Mpint::decode(reader)?;
        let p = Mpint::decode(reader)?;
        let q = Mpint::decode(reader)?;
        Ok(Self { d, iqmp, p, q })
    }
}

impl Encode for RsaPrivateKey {
    fn encoded_len(&self) -> encoding::Result<usize> {
        [
            self.d.encoded_len()?,
            self.iqmp.encoded_len()?,
            self.p.encoded_len()?,
            self.q.encoded_len()?,
        ]
        .checked_sum()
    }

    fn encode(&self, writer: &mut impl Writer) -> encoding::Result<()> {
        self.d.encode(writer)?;
        self.iqmp.encode(writer)?;
        self.p.encode(writer)?;
        self.q.encode(writer)?;
        Ok(())
    }
}

impl Drop for RsaPrivateKey {
    fn drop(&mut self) {
        self.d.zeroize();
        self.iqmp.zeroize();
        self.p.zeroize();
        self.q.zeroize();
    }
}

// Adapter between RngCore trait versions between rand_core and rsa::rand_core.
// rand_core 0.10.0 changed the trait hierarchy: TryRng -> Rng -> RngCore (deprecated).
// Only TryRng and TryCryptoRng need manual impls; Rng/CryptoRng/RngCore are blanket-impl'd.
#[cfg(feature = "rsa")]
struct RngAdapter<R: CryptoRngCore>(R);
#[cfg(feature = "rsa")]
impl<R: CryptoRngCore> rsa::rand_core::TryRng for RngAdapter<R> {
    type Error = core::convert::Infallible;

    fn try_next_u32(&mut self) -> core::result::Result<u32, Self::Error> {
        Ok(self.0.next_u32())
    }

    fn try_next_u64(&mut self) -> core::result::Result<u64, Self::Error> {
        Ok(self.0.next_u64())
    }

    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> core::result::Result<(), Self::Error> {
        self.0.fill_bytes(dst);
        Ok(())
    }
}
#[cfg(feature = "rsa")]
impl<R: CryptoRngCore> rsa::rand_core::TryCryptoRng for RngAdapter<R> {}

/// RSA private/public keypair.
#[derive(Clone)]
pub struct RsaKeypair {
    /// Public key.
    pub public: RsaPublicKey,

    /// Private key.
    pub private: RsaPrivateKey,
}

impl RsaKeypair {
    /// Minimum allowed RSA key size.
    #[cfg(all(feature = "rsa", not(feature = "hazmat-allow-insecure-rsa-keys")))]
    pub(crate) const MIN_KEY_SIZE: usize = 2048;

    /// Generate a random RSA keypair of the given size.
    #[cfg(feature = "rsa")]
    pub fn random(rng: &mut impl CryptoRngCore, bit_size: usize) -> Result<Self> {
        #[cfg(not(feature = "hazmat-allow-insecure-rsa-keys"))]
        if bit_size < Self::MIN_KEY_SIZE {
            return Err(Error::Crypto);
        }
        let mut rng = RngAdapter(rng);
        rsa::RsaPrivateKey::new(&mut rng, bit_size)?.try_into()
    }
}

impl ConstantTimeEq for RsaKeypair {
    fn ct_eq(&self, other: &Self) -> Choice {
        Choice::from((self.public == other.public) as u8) & self.private.ct_eq(&other.private)
    }
}

impl Eq for RsaKeypair {}

impl PartialEq for RsaKeypair {
    fn eq(&self, other: &Self) -> bool {
        self.ct_eq(other).into()
    }
}

impl Decode for RsaKeypair {
    type Error = Error;

    fn decode(reader: &mut impl Reader) -> Result<Self> {
        let n = Mpint::decode(reader)?;
        let e = Mpint::decode(reader)?;
        let public = RsaPublicKey { n, e };
        let private = RsaPrivateKey::decode(reader)?;
        Ok(RsaKeypair { public, private })
    }
}

impl Encode for RsaKeypair {
    fn encoded_len(&self) -> encoding::Result<usize> {
        [
            self.public.n.encoded_len()?,
            self.public.e.encoded_len()?,
            self.private.encoded_len()?,
        ]
        .checked_sum()
    }

    fn encode(&self, writer: &mut impl Writer) -> encoding::Result<()> {
        self.public.n.encode(writer)?;
        self.public.e.encode(writer)?;
        self.private.encode(writer)
    }
}

impl From<RsaKeypair> for RsaPublicKey {
    fn from(keypair: RsaKeypair) -> RsaPublicKey {
        keypair.public
    }
}

impl From<&RsaKeypair> for RsaPublicKey {
    fn from(keypair: &RsaKeypair) -> RsaPublicKey {
        keypair.public.clone()
    }
}

impl fmt::Debug for RsaKeypair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RsaKeypair")
            .field("public", &self.public)
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "rsa")]
impl TryFrom<RsaKeypair> for rsa::RsaPrivateKey {
    type Error = Error;

    fn try_from(key: RsaKeypair) -> Result<rsa::RsaPrivateKey> {
        rsa::RsaPrivateKey::try_from(&key)
    }
}

#[cfg(feature = "rsa")]
impl TryFrom<&RsaKeypair> for rsa::RsaPrivateKey {
    type Error = Error;

    fn try_from(key: &RsaKeypair) -> Result<rsa::RsaPrivateKey> {
        use rsa::BoxedUint;

        let ret: rsa::RsaPrivateKey = rsa::RsaPrivateKey::from_components(
            BoxedUint::try_from(&key.public.n)?,
            BoxedUint::try_from(&key.public.e)?,
            BoxedUint::try_from(&key.private.d)?,
            vec![
                BoxedUint::try_from(&key.private.p)?,
                BoxedUint::try_from(&key.private.q)?,
            ],
        )?;

        #[cfg(not(feature = "hazmat-allow-insecure-rsa-keys"))]
        if ret.size().saturating_mul(8) < RsaKeypair::MIN_KEY_SIZE {
            return Err(Error::Crypto);
        }

        Ok(ret)
    }
}

#[cfg(feature = "rsa")]
impl TryFrom<rsa::RsaPrivateKey> for RsaKeypair {
    type Error = Error;

    fn try_from(key: rsa::RsaPrivateKey) -> Result<RsaKeypair> {
        RsaKeypair::try_from(&key)
    }
}

#[cfg(feature = "rsa")]
impl TryFrom<&rsa::RsaPrivateKey> for RsaKeypair {
    type Error = Error;

    fn try_from(key: &rsa::RsaPrivateKey) -> Result<RsaKeypair> {
        // Multi-prime keys are not supported
        if key.primes().len() > 2 {
            return Err(Error::Crypto);
        }

        let public = RsaPublicKey::try_from(key.to_public_key())?;

        let p = &key.primes()[0];
        let q = &key.primes()[1];
        let iqmp = key.crt_coefficient().ok_or(Error::Crypto)?;

        let private = RsaPrivateKey {
            d: key.d().try_into()?,
            iqmp: iqmp.try_into()?,
            p: p.try_into()?,
            q: q.try_into()?,
        };

        Ok(RsaKeypair { public, private })
    }
}

#[cfg(feature = "rsa")]
impl<D> TryFrom<&RsaKeypair> for pkcs1v15::SigningKey<D>
where
    D: digest_next::Digest + AssociatedOid,
{
    type Error = Error;

    fn try_from(keypair: &RsaKeypair) -> Result<pkcs1v15::SigningKey<D>> {
        Ok(pkcs1v15::SigningKey::new(keypair.try_into()?))
    }
}
