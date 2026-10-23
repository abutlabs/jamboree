//! A GP 0.8.0 work-package (eq. workpackage, workitem, workcontext), the authorizer that
//! opens it, and a service's account record: what a client-neutral builder needs.

use super::codec::{blake2b256, enc_nat, var};
use anyhow::{Result, bail};

/// Who authorizes a package: the service `host` holding the authorizer code as a preimage,
/// that code's hash, its configuration, and the token the package carries. Its identity in
/// a core's pool is H(code_hash ++ config).
#[derive(Clone, Debug, PartialEq)]
pub struct Authorizer {
    pub host: u32,
    pub code_hash: [u8; 32],
    pub config: Vec<u8>,
    pub token: Vec<u8>,
}

impl Authorizer {
    pub fn hash(&self) -> [u8; 32] {
        let mut b = self.code_hash.to_vec();
        b.extend_from_slice(&self.config);
        blake2b256(&b)
    }

    /// `host:code_hash[:config[:token]]`, hex blobs.
    pub fn parse(text: &str) -> Result<Self> {
        let parts: Vec<&str> = text.trim().split(':').collect();
        if !(2..=4).contains(&parts.len()) {
            bail!("authorizer {text:?}: expected host:code_hash[:config[:token]]");
        }
        let blob = |s: &str| hex::decode(s.trim_start_matches("0x")).map_err(anyhow::Error::from);
        let code_hash: [u8; 32] = blob(parts[1])?.try_into().map_err(|_| anyhow::anyhow!("authorizer code hash: 32 bytes"))?;
        Ok(Self {
            host: parts[0].parse()?,
            code_hash,
            config: parts.get(2).map(|p| blob(p)).transpose()?.unwrap_or_default(),
            token: parts.get(3).map(|p| blob(p)).transpose()?.unwrap_or_default(),
        })
    }
}

impl std::fmt::Display for Authorizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.host, hex::encode(self.code_hash))
    }
}

/// The refinement context: the anchor and its state and BEEFY roots, the lookup anchor and
/// its state root; no prerequisites.
pub struct RefineContext {
    pub anchor: [u8; 32],
    pub anchor_slot: u32,
    pub state_root: [u8; 32],
    pub beefy_root: [u8; 32],
    pub lookup_anchor: [u8; 32],
    pub lookup_anchor_slot: u32,
    pub lookup_anchor_state_root: [u8; 32],
}

impl RefineContext {
    fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(200);
        b.extend_from_slice(&self.anchor);
        b.extend_from_slice(&self.anchor_slot.to_le_bytes());
        b.extend_from_slice(&self.state_root);
        b.extend_from_slice(&self.beefy_root);
        b.extend_from_slice(&self.lookup_anchor);
        b.extend_from_slice(&self.lookup_anchor_slot.to_le_bytes());
        b.extend_from_slice(&self.lookup_anchor_state_root);
        b.extend(enc_nat(0)); // prerequisites
        b
    }
}

/// One work-item for `service`: its code hash, the payload, the gas limits, and its extrinsics
/// (data that travels beside the package, not in it: JAM-NP limits the package to 200 KiB,
/// while extrinsics count only toward W_B); no imports or exports.
pub struct WorkItem {
    pub service: u32,
    pub code_hash: [u8; 32],
    pub payload: Vec<u8>,
    pub refine_gas: u64,
    pub accumulate_gas: u64,
    pub extrinsics: Vec<Vec<u8>>,
    /// how many segments refine will export (GP: the report must export exactly this many)
    pub export_count: u16,
    /// segments to import: (hash, index, the hash is a work-package hash rather than a segments root)
    pub imports: Vec<Import>,
}

/// One imported segment (GP C.30, I(h, i)): a segments root and an index, or a work-package
/// hash (the tagged form, h^+) whose index carries 2^15; the guarantor resolves it.
#[derive(Clone, Debug, PartialEq)]
pub struct Import {
    pub hash: [u8; 32],
    pub index: u16,
    pub by_package: bool,
}

impl Import {
    /// "root:<hex>:<i>" or "pkg:<hex>:<i>"
    pub fn parse(s: &str) -> Result<Self> {
        let mut p = s.splitn(3, ':');
        let (kind, h, i) = (p.next().unwrap_or(""), p.next().unwrap_or(""), p.next().unwrap_or(""));
        let by_package = match kind { "root" => false, "pkg" => true, _ => bail!("import {s}: root:<hash>:<i> or pkg:<hash>:<i>") };
        let raw = hex::decode(h.trim_start_matches("0x"))?;
        let hash: [u8; 32] = raw.try_into().map_err(|_| anyhow::anyhow!("import {s}: a 32-byte hash"))?;
        let index: u16 = i.parse()?;
        if index >= 1 << 15 { bail!("import {s}: index below 2^15"); }
        Ok(Import { hash, index, by_package })
    }

    fn encode(&self) -> Vec<u8> {
        let mut b = self.hash.to_vec();
        b.extend_from_slice(&(self.index | if self.by_package { 1 << 15 } else { 0 }).to_le_bytes());
        b
    }
}

impl WorkItem {
    fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(64 + self.payload.len());
        b.extend_from_slice(&self.service.to_le_bytes());
        b.extend_from_slice(&self.code_hash);
        b.extend_from_slice(&self.refine_gas.to_le_bytes());
        b.extend_from_slice(&self.accumulate_gas.to_le_bytes());
        b.extend_from_slice(&self.export_count.to_le_bytes());
        b.extend(var(&self.payload));
        b.extend(enc_nat(self.imports.len() as u64));
        for i in &self.imports {
            b.extend(i.encode());
        }
        // extrinsics, by reference: GP C.29, (blake2b-256 of the data, E4(length)) each
        b.extend(enc_nat(self.extrinsics.len() as u64));
        for x in &self.extrinsics {
            b.extend_from_slice(&blake2b256(x));
            b.extend_from_slice(&(x.len() as u32).to_le_bytes());
        }
        b
    }
}

pub fn encode_package(auth: &Authorizer, ctx: &RefineContext, items: &[WorkItem]) -> Result<Vec<u8>> {
    if items.is_empty() {
        bail!("a work-package carries at least one work-item");
    }
    let mut b = Vec::new();
    b.extend_from_slice(&auth.host.to_le_bytes());
    b.extend_from_slice(&auth.code_hash);
    b.extend(ctx.encode());
    b.extend(var(&auth.token));
    b.extend(var(&auth.config));
    b.extend(enc_nat(items.len() as u64));
    for w in items {
        b.extend(w.encode());
    }
    Ok(b)
}

/// A service account record, C(255, s): 89 bytes, E(0, code_hash, E8(balance, min_item_gas,
/// min_memo_gas, octets, gratis), E4(items, created, last_accumulated, parent)).
#[derive(Debug)]
pub struct ServiceInfo {
    pub code_hash: [u8; 32],
    pub balance: u64,
    pub min_item_gas: u64,
    #[allow(dead_code)] // decoded for completeness: the record's layout
    pub min_memo_gas: u64,
    pub octets: u64,
    pub items: u32,
    pub created: u32,
    pub last_accumulated: u32,
    pub parent: u32,
}

impl ServiceInfo {
    pub fn decode(d: &[u8]) -> Result<Self> {
        if d.len() != 89 || d[0] != 0 {
            bail!("service record: expected 89 bytes with version 0, got {} bytes", d.len());
        }
        let e8 = |i: usize| u64::from_le_bytes(d[33 + 8 * i..41 + 8 * i].try_into().unwrap());
        let e4 = |i: usize| u32::from_le_bytes(d[73 + 4 * i..77 + 4 * i].try_into().unwrap());
        Ok(Self {
            code_hash: d[1..33].try_into().unwrap(),
            balance: e8(0),
            min_item_gas: e8(1),
            min_memo_gas: e8(2),
            octets: e8(3),
            items: e4(0),
            created: e4(1),
            last_accumulated: e4(2),
            parent: e4(3),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorizer_text_round_trips() {
        let a = Authorizer::parse(&format!("7:{}", "ab".repeat(32))).unwrap();
        assert_eq!((a.host, a.config.len(), a.token.len()), (7, 0, 0));
        assert_eq!(a.to_string(), format!("7:{}", "ab".repeat(32)));
        assert!(Authorizer::parse("7:abcd").is_err());
    }

    #[test]
    fn package_layout_matches_the_python_builder() {
        // the layout jamswap's workpackage.py produces, byte for byte (its fixed vector)
        let auth = Authorizer { host: 0, code_hash: [1; 32], config: vec![], token: vec![] };
        let ctx = RefineContext {
            anchor: [2; 32], anchor_slot: 5, state_root: [3; 32], beefy_root: [4; 32],
            lookup_anchor: [5; 32], lookup_anchor_slot: 4, lookup_anchor_state_root: [6; 32],
        };
        let item = WorkItem { service: 9, code_hash: [7; 32], payload: vec![0xaa, 0xbb], refine_gas: 10, accumulate_gas: 20, extrinsics: vec![], export_count: 0, imports: vec![] };
        let p = encode_package(&auth, &ctx, &[item]).unwrap();
        // jamswap's workpackage.py encodes the same package to exactly these bytes
        assert_eq!(hex::encode(p), "000000000101010101010101010101010101010101010101010101010101010101010101020202020202020202020202020202020202020202020202020202020202020205000000030303030303030303030303030303030303030303030303030303030303030304040404040404040404040404040404040404040404040404040404040404040505050505050505050505050505050505050505050505050505050505050505040000000606060606060606060606060606060606060606060606060606060606060606000000010900000007070707070707070707070707070707070707070707070707070707070707070a000000000000001400000000000000000002aabb0000");
    }

    #[test]
    fn extrinsics_are_referenced_by_hash_and_length() {
        // GP C.29: a work-item ends with its extrinsics as (blake2b-256, E4(length)) pairs
        let xs = vec![vec![1u8; 300], vec![2u8; 5]];
        let item = WorkItem { service: 9, code_hash: [7; 32], payload: vec![0xaa], refine_gas: 10, accumulate_gas: 20, extrinsics: xs.clone(), export_count: 0, imports: vec![] };
        let enc = item.encode();
        let mut tail = enc_nat(2);
        for x in &xs {
            tail.extend_from_slice(&blake2b256(x));
            tail.extend_from_slice(&(x.len() as u32).to_le_bytes());
        }
        assert!(enc.ends_with(&tail));
        let none = WorkItem { extrinsics: vec![], ..item };
        assert_eq!(enc.len() - none.encode().len(), 2 * 36);
    }

    #[test]
    fn imports_by_root_and_by_package() {
        // GP C.30: (h, E2(i)) for a segments root, (r, E2(i + 2^15)) for a work-package hash
        let r = Import::parse(&format!("root:{}:5", "11".repeat(32))).unwrap();
        let p = Import::parse(&format!("pkg:0x{}:5", "22".repeat(32))).unwrap();
        assert_eq!(&r.encode()[32..], &5u16.to_le_bytes());
        assert_eq!(&p.encode()[32..], &(5u16 + (1 << 15)).to_le_bytes());
        assert!(Import::parse(&format!("pkg:{}:32768", "22".repeat(32))).is_err());
        let item = WorkItem { service: 1, code_hash: [0; 32], payload: vec![], refine_gas: 1, accumulate_gas: 1,
                              extrinsics: vec![], export_count: 0, imports: vec![r.clone(), p.clone()] };
        let enc = item.encode();
        let mut want = enc_nat(2);
        want.extend(r.encode());
        want.extend(p.encode());
        want.extend(enc_nat(0)); // no extrinsics
        assert!(enc.ends_with(&want));
    }
}
