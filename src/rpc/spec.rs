//! What a builder needs from a JIP-4 chain spec: the authorizer a work-package can use.
//! JIP-2 has no call that reads the authorizer pools, so the genesis state is where a
//! client-neutral builder learns them: C(1) holds the pools; for every service s and every
//! preimage p it holds in genesis, if H(H(p)) (the empty configuration) is in some core's
//! pool, then (s, H(p)) authorizes packages on those cores. A port of jamswap's chainspec.py.

use super::codec::{blake2b256, dec_nat};
use super::wp::Authorizer;
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::collections::BTreeMap;

/// E4(2^32 - 2): the key prefix of a preimage held by a service.
const PREIMAGE_TAG: [u8; 4] = [0xfe, 0xff, 0xff, 0xff];

/// C(s, h): the service id's four octets interleaved with H(h)'s first four, then 23 more.
fn state_key(s: u32, h: &[u8]) -> [u8; 31] {
    let (n, a) = (s.to_le_bytes(), blake2b256(h));
    let mut k = [0u8; 31];
    k[..8].copy_from_slice(&[n[0], a[0], n[1], a[1], n[2], a[2], n[3], a[3]]);
    k[8..].copy_from_slice(&a[4..27]);
    k
}

fn genesis_state(spec: &Value) -> Result<BTreeMap<Vec<u8>, Vec<u8>>> {
    let unhex = |s: &str| hex::decode(s.trim_start_matches("0x"));
    let obj = spec["genesis_state"].as_object().context("chain spec: no genesis_state object")?;
    let mut out = BTreeMap::new();
    for (k, v) in obj {
        let key = unhex(k)?;
        if key.len() != 31 {
            bail!("chain spec: state keys must be 31 bytes: {k}");
        }
        out.insert(key, unhex(v.as_str().context("chain spec: a state value is not a string")?)?);
    }
    Ok(out)
}

/// (the authorizer, the cores whose genesis pool accepts it): the one accepted on the most
/// cores, then the lowest host id.
pub fn authorizer(spec: &Value) -> Result<(Authorizer, Vec<u32>)> {
    let state = genesis_state(spec)?;
    let mut pools_key = vec![1u8];
    pools_key.extend([0u8; 30]);
    let raw = state.get(&pools_key).context("chain spec: no authorizer pools (C(1)) in genesis")?;
    let mut pools: Vec<Vec<[u8; 32]>> = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let (n, j) = dec_nat(raw, i)?;
        let n = n as usize;
        if j + 32 * n > raw.len() {
            bail!("chain spec: truncated authorizer pool");
        }
        pools.push((0..n).map(|k| raw[j + 32 * k..j + 32 * k + 32].try_into().unwrap()).collect());
        i = j + 32 * n;
    }
    // services: account records C(255, s) = [255, n0, 0, n1, 0, n2, 0, n3, 0, 0...]
    let services: Vec<u32> = state.keys()
        .filter(|k| k[0] == 255 && k[2..8].iter().step_by(2).all(|b| *b == 0) && k[8..].iter().all(|b| *b == 0))
        .map(|k| u32::from_le_bytes([k[1], k[3], k[5], k[7]]))
        .collect();
    let mut found: Vec<(Authorizer, Vec<u32>)> = Vec::new();
    for &s in &services {
        for (key, blob) in &state {
            let code_hash = blake2b256(blob);
            let mut tagged = PREIMAGE_TAG.to_vec();
            tagged.extend_from_slice(&code_hash);
            if key.as_slice() != state_key(s, &tagged) {
                continue;
            }
            let auth = Authorizer { host: s, code_hash, config: vec![], token: vec![] };
            let h = auth.hash();
            let cores: Vec<u32> = pools.iter().enumerate().filter(|(_, p)| p.contains(&h)).map(|(c, _)| c as u32).collect();
            if !cores.is_empty() {
                found.push((auth, cores));
            }
        }
    }
    found.sort_by_key(|(a, c)| (std::cmp::Reverse(c.len()), a.host));
    found.into_iter().next().context("chain spec: no genesis preimage is an empty-configuration authorizer in any core's pool; name one with --authorizer")
}
