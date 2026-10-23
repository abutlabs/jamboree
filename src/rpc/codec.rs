//! The GP 0.8.0 codec pieces a work-package needs (appendix C), and blake2b-256.

use anyhow::{Result, bail};
use blake2::{Blake2b, Digest, digest::consts::U32};

pub fn blake2b256(data: &[u8]) -> [u8; 32] {
    let mut h = Blake2b::<U32>::new();
    h.update(data);
    h.finalize().into()
}

/// The general natural serialization E(x): 1 to 9 octets.
pub fn enc_nat(x: u64) -> Vec<u8> {
    if x == 0 {
        return vec![0];
    }
    for l in 0..8u32 {
        if x < 1u64 << (7 * (l + 1)) {
            let prefix = (256 - (1u32 << (8 - l)) + (x >> (8 * l)) as u32) as u8;
            let mut out = vec![prefix];
            out.extend_from_slice(&x.to_le_bytes()[..l as usize]);
            return out;
        }
    }
    let mut out = vec![0xff];
    out.extend_from_slice(&x.to_le_bytes());
    out
}

/// Decode E(x) at `buf[i..]`: (x, next index). Refuses non-canonical encodings.
pub fn dec_nat(buf: &[u8], i: usize) -> Result<(u64, usize)> {
    let Some(&b) = buf.get(i) else { bail!("truncated natural") };
    let l = if b == 0xff { 8 } else { (0..8).find(|n| b & (0x80 >> n) == 0).unwrap_or(8) };
    if i + 1 + l > buf.len() {
        bail!("truncated natural");
    }
    let mut rest = [0u8; 8];
    rest[..l].copy_from_slice(&buf[i + 1..i + 1 + l]);
    let rest = u64::from_le_bytes(rest);
    let x = if l == 8 { rest } else { rest + (((b as u64) & ((1 << (7 - l)) - 1)) << (8 * l)) };
    if enc_nat(x) != buf[i..i + 1 + l] {
        bail!("non-canonical natural {}", hex::encode(&buf[i..i + 1 + l]));
    }
    Ok((x, i + 1 + l))
}

/// var(blob): its length as E, then the blob.
pub fn var(blob: &[u8]) -> Vec<u8> {
    let mut out = enc_nat(blob.len() as u64);
    out.extend_from_slice(blob);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn naturals_round_trip_at_every_width() {
        for x in [0u64, 1, 127, 128, 16_383, 16_384, 1 << 21, (1 << 28) - 1, 1 << 35, 1 << 56, u64::MAX] {
            let e = enc_nat(x);
            assert_eq!(dec_nat(&e, 0).unwrap(), (x, e.len()), "x = {x}");
        }
        assert_eq!(enc_nat(128), vec![0x80, 0x80]);
        assert!(dec_nat(&[0x80, 0x05], 0).is_err(), "5 fits one octet: non-canonical");
    }

    #[test]
    fn blake2b256_of_empty() {
        assert_eq!(hex::encode(blake2b256(b"")), "0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a8");
    }
}
