//! `jamboree rpc`: talk to a running JAM network over JIP-2. Read state, build and submit
//! work-packages, follow their status, and deploy a service through the chain's Bootstrap
//! service, from the chain spec alone: no client-specific tooling. A Rust port of the
//! client-neutral builder in abutlabs/jamswap (offchain/jip2.py, workpackage.py, chain.py,
//! chainspec.py, deploy.py).

mod client;
mod codec;
mod spec;
mod wp;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use client::{Block, Client, RpcError};
use codec::{blake2b256, dec_nat, enc_nat};
use serde_json::{Value, json};
use std::{path::PathBuf, time::{Duration, Instant}};
use wp::{Authorizer, RefineContext, ServiceInfo, WorkItem, encode_package};

#[derive(Parser, Debug)]
#[command(name = "jamboree rpc", about = "Talk to a running JAM network over JIP-2: read state, submit work-packages, deploy services", max_term_width = 100)]
struct RpcCli {
    /// The node's JIP-2 WebSocket
    #[arg(long, default_value = "ws://127.0.0.1:19800", global = true)]
    url: String,
    /// A JIP-4 chain spec: its genesis names the authorizer packages use
    #[arg(long, global = true)]
    chain_spec: Option<PathBuf>,
    /// The authorizer, host:code_hash[:config[:token]] (instead of --chain-spec)
    #[arg(long, global = true)]
    authorizer: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// The best and finalized blocks
    Head,
    /// The authorizer packages will use, and its cores (from --chain-spec or --authorizer)
    Authorizer,
    /// The chain parameters (JIP-2 parameters, V1)
    Params,
    /// The services at the best block
    Services,
    /// A service's account record
    Service { id: u32 },
    /// A storage value: KEY is 0x-prefixed hex, or text; prints hex or (none)
    Read { service: u32, key: String },
    /// One work-item for SERVICE with PAYLOAD (0x-hex, or @file of raw bytes) in its own package;
    /// prints the receipt as JSON
    Submit {
        service: u32,
        payload: String,
        /// Extrinsic data for the work-item (0x-hex, or @file), repeatable; it travels beside the
        /// package, so it is not held to JAM-NP's 200 KiB package limit
        #[arg(long)]
        extrinsic: Vec<String>,
        /// Segments the work-item's refine will export (the report must match it)
        #[arg(long, default_value_t = 0)]
        exports: u16,
        /// A segment to import, root:<hash>:<i> (a segments root) or pkg:<hash>:<i> (a
        /// work-package hash, resolved by the guarantor); repeatable
        #[arg(long)]
        import: Vec<String>,
        /// Only this core (a chain of dependent packages stays on one core)
        #[arg(long)]
        core: Option<u32>,
        /// Wait until JIP-2 reports the package Reported, Ready or Failed
        #[arg(long)]
        wait: bool,
        /// Seconds to wait with --wait
        #[arg(long, default_value_t = 120)]
        timeout: u64,
    },
    /// JIP-2 workPackageStatus of a package, at the best block
    Status { package: String, anchor: String },
    /// JIP-2 workReport: a report by its hash (hex or base64, as Status prints it); prints its
    /// package spec: package hash, bundle length, erasure root, segments root, export count
    Report { hash: String },
    /// Deploy a service blob through the Bootstrap service; prints SERVICE_ID=<id>
    Deploy {
        code: PathBuf,
        /// Ask for this id (below 2^16); default the lowest free one
        #[arg(long)]
        id: Option<u32>,
        /// Remember the deployment here (JSON), and reuse it if the chain still runs this code
        #[arg(long)]
        state: Option<PathBuf>,
        /// Deploy anew even if a service already runs this code
        #[arg(long)]
        fresh: bool,
        #[arg(long, default_value_t = 1_000_000_000)]
        endowment: u64,
        #[arg(long, default_value_t = 0)]
        bootstrap: u32,
        #[arg(long, default_value_t = 600)]
        timeout: u64,
    },
}

/// Called from main when the first argument is `rpc`.
pub fn main(args: Vec<String>) -> Result<()> {
    let cli = RpcCli::parse_from(args);
    let mut c = Chain::new(&cli.url, cli.chain_spec.as_deref(), cli.authorizer.as_deref())?;
    match cli.cmd {
        Cmd::Head => {
            let (b, f) = (c.rpc.best_block()?, c.rpc.finalized_block()?);
            println!("best {} 0x{} finalized {} 0x{}", b.slot, hex::encode(b.hash), f.slot, hex::encode(f.hash));
        }
        Cmd::Authorizer => {
            let (a, cores) = c.authorizer()?;
            println!("{a} cores {}", cores.iter().map(u32::to_string).collect::<Vec<_>>().join(","));
        }
        Cmd::Params => println!("{}", serde_json::to_string_pretty(&c.rpc.parameters()?)?),
        Cmd::Services => {
            let at = c.rpc.best_block()?.hash;
            println!("{}", c.rpc.list_services(&at)?.iter().map(u32::to_string).collect::<Vec<_>>().join(" "));
        }
        Cmd::Service { id } => match c.record(id, None)? {
            Some(i) => println!("service {id}: code 0x{} balance {} min_item_gas {} items {} octets {} created {} last_accumulated {} parent {}",
                hex::encode(i.code_hash), i.balance, i.min_item_gas, i.items, i.octets, i.created, i.last_accumulated, i.parent),
            None => println!("(none)"),
        },
        Cmd::Read { service, key } => {
            let at = c.rpc.best_block()?.hash;
            match c.rpc.service_value(&at, service, &bytes_arg(&key, false)?)? {
                Some(v) => println!("{}", hex::encode(v)),
                None => println!("(none)"),
            }
        }
        Cmd::Submit { service, payload, extrinsic, exports, import, core, wait, timeout } => {
            let xs = extrinsic.iter().map(|x| bytes_arg(x, true)).collect::<Result<Vec<_>>>()?;
            let imports = import.iter().map(|s| wp::Import::parse(s)).collect::<Result<Vec<_>>>()?;
            let r = c.submit(service, &bytes_arg(&payload, true)?, &xs, exports, &imports, core)?;
            println!("{r}");
            if wait {
                let (package, anchor) = (hex32(r["package_hash"].as_str().unwrap())?, hex32(r["anchor"].as_str().unwrap())?);
                let t0 = Instant::now();
                loop {
                    let st = c.status(&package, &anchor)?;
                    let kind = st.as_object().and_then(|o| o.keys().next().cloned()).unwrap_or_default();
                    if matches!(kind.as_str(), "Reported" | "Ready" | "Failed") {
                        println!("{}", json!({"status": st}));
                        if kind == "Failed" { bail!("package failed: {st}"); }
                        break;
                    }
                    if t0.elapsed() > Duration::from_secs(timeout) { bail!("no Reported, Ready or Failed in {timeout} s (last: {st})"); }
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
        Cmd::Status { package, anchor } => println!("{}", json!({"status": c.status(&hex32(&package)?, &hex32(&anchor)?)?})),
        Cmd::Report { hash } => {
            let h = hex32(&hash).or_else(|_| client::hash32(&Value::String(hash.clone())))?;
            let r = client::unb64(&c.rpc.call("workReport", json!([client::b64(&h)]), false)?)?;
            // GP 0.8.0 C (serialization.tex 208-215): a report opens with its package spec,
            // (h, E4(bundle length), erasure root, E2(erasure shards), segments root, E2(exports))
            if r.len() < 104 { bail!("workReport: {} bytes, too short for a package spec", r.len()); }
            println!("{}", json!({"package_hash": hex::encode(&r[0..32]),
                "bundle_length": u32::from_le_bytes(r[32..36].try_into()?), "erasure_root": hex::encode(&r[36..68]),
                "erasure_shards": u16::from_le_bytes(r[68..70].try_into()?),
                "segments_root": hex::encode(&r[70..102]), "exports": u16::from_le_bytes(r[102..104].try_into()?)}));
        }
        Cmd::Deploy { code, id, state, fresh, endowment, bootstrap, timeout } => {
            let blob = std::fs::read(&code).with_context(|| format!("read {}", code.display()))?;
            let sid = Deploy { c: &mut c, code: blob, bootstrap, deadline: Instant::now() + Duration::from_secs(timeout) }
                .run(id, fresh, endowment, state.as_deref())?;
            println!("SERVICE_ID={sid}");
        }
    }
    Ok(())
}

fn bytes_arg(s: &str, file_ok: bool) -> Result<Vec<u8>> {
    if let Some(path) = s.strip_prefix('@').filter(|_| file_ok) {
        return std::fs::read(path).with_context(|| format!("read {path}"));
    }
    match s.strip_prefix("0x") {
        Some(h) => Ok(hex::decode(h)?),
        None if file_ok => Ok(hex::decode(s).context("payload: 0x-hex, plain hex, or @file")?),
        None => Ok(s.as_bytes().to_vec()),
    }
}

fn hex32(s: &str) -> Result<[u8; 32]> {
    hex::decode(s.trim_start_matches("0x"))?.try_into().map_err(|_| anyhow!("expected a 32-byte hex hash: {s}"))
}

/// A connection plus what package building needs: the parameters and the authorizer.
struct Chain {
    rpc: Client,
    params: Option<Value>,
    spec: Option<Value>,
    auth: Option<(Authorizer, Vec<u32>)>,
    next_core: usize,
}

impl Chain {
    fn new(url: &str, spec: Option<&std::path::Path>, authorizer: Option<&str>) -> Result<Self> {
        let spec = spec.map(|p| -> Result<Value> {
            Ok(serde_json::from_slice(&std::fs::read(p).with_context(|| format!("read {}", p.display()))?)?)
        }).transpose()?;
        let auth = authorizer.map(|a| Authorizer::parse(a).map(|a| (a, vec![]))).transpose()?;
        Ok(Self { rpc: Client::new(url), params: None, spec, auth, next_core: 0 })
    }

    fn param(&mut self, name: &str) -> Result<u64> {
        if self.params.is_none() {
            self.params = Some(self.rpc.parameters()?);
        }
        let v = &self.params.as_ref().unwrap()[name];
        v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).ok_or_else(|| anyhow!("parameters: no number {name}"))
    }

    fn authorizer(&mut self) -> Result<(Authorizer, Vec<u32>)> {
        if self.auth.is_none() {
            let spec = self.spec.as_ref().ok_or_else(|| anyhow!("no authorizer to submit with: give --chain-spec or --authorizer"))?;
            self.auth = Some(spec::authorizer(spec)?);
        }
        let (a, mut cores) = self.auth.clone().unwrap();
        if cores.is_empty() {
            cores = (0..self.param("core_count")? as u32).collect();
        }
        Ok((a, cores))
    }

    fn record(&mut self, service: u32, at: Option<[u8; 32]>) -> Result<Option<ServiceInfo>> {
        let at = match at { Some(h) => h, None => self.rpc.best_block()?.hash };
        self.rpc.service_data(&at, service)?.map(|d| ServiceInfo::decode(&d)).transpose()
    }

    /// The anchor: the best block's parent, the newest block whose recorded state root is
    /// final (recent history holds the newest block with a zero state root until its child).
    fn anchor(&mut self) -> Result<Block> {
        let best = self.rpc.best_block()?;
        self.rpc.parent(best)
    }

    /// The lookup anchor: the finalized block's parent, unless finality lags so far that it
    /// would be too old when the report lands; then the anchor itself.
    fn lookup_anchor(&mut self, anchor: Block) -> Result<Block> {
        let fin = self.rpc.finalized_block()?;
        let lookup = self.rpc.parent(fin)?;
        let margin = self.param("max_lookup_anchor_age")?.saturating_sub(self.param("recent_block_count")?);
        Ok(if (lookup.slot as u64) + margin < anchor.slot as u64 { anchor } else { lookup })
    }

    /// One package carrying one work-item for `service`, with all the gas a package allows.
    fn package(&mut self, service: u32, payload: &[u8], extrinsics: &[Vec<u8>], export_count: u16, imports: &[wp::Import]) -> Result<(Vec<u8>, Block)> {
        let (auth, _) = self.authorizer()?;
        let anchor = self.anchor()?;
        let lookup = self.lookup_anchor(anchor)?;
        let state_root = self.rpc.state_root(&anchor.hash)?;
        let ctx = RefineContext {
            anchor: anchor.hash, anchor_slot: anchor.slot, state_root, beefy_root: self.rpc.beefy_root(&anchor.hash)?,
            lookup_anchor: lookup.hash, lookup_anchor_slot: lookup.slot,
            lookup_anchor_state_root: if lookup.hash == anchor.hash { state_root } else { self.rpc.state_root(&lookup.hash)? },
        };
        let info = self.record(service, Some(anchor.hash))?.ok_or_else(|| anyhow!("no service {service} at the anchor"))?;
        let (g_r, g_a) = (self.param("max_refine_gas")?, self.param("max_accumulate_gas")?);
        if g_a - 1 < info.min_item_gas {
            bail!("service {service} wants {} accumulate gas per item; a package allows {}", info.min_item_gas, g_a - 1);
        }
        let item = WorkItem { service, code_hash: info.code_hash, payload: payload.to_vec(), refine_gas: g_r - 1, accumulate_gas: g_a - 1,
                          extrinsics: extrinsics.to_vec(), export_count, imports: imports.to_vec() };
        Ok((encode_package(&auth, &ctx, &[item])?, anchor))
    }

    /// Submit to the next core (round robin over those the authorizer's pool covers, or `core`).
    fn submit(&mut self, service: u32, payload: &[u8], extrinsics: &[Vec<u8>], export_count: u16, imports: &[wp::Import], core: Option<u32>) -> Result<Value> {
        let (package, anchor) = self.package(service, payload, extrinsics, export_count, imports)?;
        let cores = match core { Some(c) => vec![c], None => self.authorizer()?.1 };
        let mut refused = vec![];
        for k in 0..cores.len() {
            let c = cores[(self.next_core + k) % cores.len()];
            match self.rpc.submit_work_package(c, &package, extrinsics) {
                Ok(_) => {
                    self.next_core = (self.next_core + k + 1) % cores.len();
                    return Ok(json!({"accepted": true, "package_hash": hex::encode(blake2b256(&package)), "core": c,
                        "anchor": hex::encode(anchor.hash), "anchor_slot": anchor.slot, "bytes": package.len(), "refused": refused}));
                }
                Err(e) => match e.downcast_ref::<RpcError>() {
                    Some(r) if r.code == -32601 => bail!("the node has no submitWorkPackage: {e}"),
                    Some(_) => refused.push(format!("core {c}: {e}")),
                    None => return Err(e.context("submitWorkPackage: outcome unknown")),
                },
            }
        }
        bail!("every core refused the package: {}", refused.join("; "))
    }

    fn status(&mut self, package: &[u8; 32], anchor: &[u8; 32]) -> Result<Value> {
        let at = self.rpc.best_block()?.hash;
        self.rpc.work_package_status(&at, package, anchor)
    }
}

// ---- deploy through the Bootstrap service ------------------------------------------------

/// jam-bootstrap-service 0.1.29 `Instruction::CreateService`: variant 0, code hash and
/// length, min item and memo gas, endowment, a W_T-octet memo, an optional registration,
/// the deposit offset, an optional id. The payload is E(n), the instructions, a 32-byte salt.
fn create_service_payload(code_hash: &[u8; 32], code_len: u64, endowment: u64, memo_size: usize, id: Option<u32>) -> Vec<u8> {
    let mut i = vec![0u8];
    i.extend_from_slice(code_hash);
    i.extend_from_slice(&code_len.to_le_bytes());
    i.extend_from_slice(&10_000u64.to_le_bytes()); // min_item_gas (jamt's default)
    i.extend_from_slice(&10_000u64.to_le_bytes()); // min_memo_gas
    i.extend_from_slice(&endowment.to_le_bytes());
    i.extend(std::iter::repeat_n(0u8, memo_size));
    i.push(0); // registration: none
    i.extend_from_slice(&0u64.to_le_bytes()); // deposit offset
    match id { Some(s) => { i.push(1); i.extend_from_slice(&s.to_le_bytes()); } None => i.push(0) }
    let mut p = enc_nat(1);
    p.extend(i);
    p.extend(rand::random::<[u8; 32]>()); // a salt: two identical creates make different packages
    p
}

/// "name version" from a code blob's conventional metadata, E(var(m), c) with m = 0, name, version.
fn describe_code(blob: &[u8]) -> String {
    let f = || -> Result<String> {
        let (n, i) = dec_nat(blob, 0)?;
        let meta = blob.get(i..i + n as usize).ok_or_else(|| anyhow!("short"))?;
        if meta.first() != Some(&0) { bail!("no metadata") }
        let (l, j) = dec_nat(meta, 1)?;
        let name = std::str::from_utf8(&meta[j..j + l as usize])?;
        let (l2, k) = dec_nat(meta, j + l as usize)?;
        Ok(format!("{name} {}", std::str::from_utf8(&meta[k..k + l2 as usize])?))
    };
    f().unwrap_or_default()
}

struct Deploy<'a> {
    c: &'a mut Chain,
    code: Vec<u8>,
    bootstrap: u32,
    deadline: Instant,
}

impl Deploy<'_> {
    fn ours(&self, i: &Option<ServiceInfo>) -> bool {
        i.as_ref().is_some_and(|i| i.code_hash == blake2b256(&self.code))
    }

    fn wait<T>(&mut self, what: &str, mut check: impl FnMut(&mut Self) -> Result<Option<T>>) -> Result<T> {
        let mut last = None;
        loop {
            match check(self) {
                Ok(Some(v)) => return Ok(v),
                Ok(None) => {}
                Err(e) => last = Some(e),
            }
            if Instant::now() > self.deadline {
                bail!("timed out waiting for {what}{}", last.map(|e| format!(" (last error: {e})")).unwrap_or_default());
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    fn run(mut self, id: Option<u32>, fresh: bool, endowment: u64, state: Option<&std::path::Path>) -> Result<u32> {
        let hash = blake2b256(&self.code);
        let reuse = if fresh || id.is_some() { None } else {
            let from_state = state.and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .filter(|v| v["code_hash"].as_str() == Some(&hex::encode(hash))).and_then(|v| v["service_id"].as_u64()).map(|s| s as u32);
            match from_state { Some(s) if { let r = self.c.record(s, None)?; self.ours(&r) } => Some(s), _ => None }
        };
        let sid = match reuse {
            Some(s) => { eprintln!("deploy: reusing service {s}"); s }
            None => {
                eprintln!("deploy: {} ({} bytes, hash {}..)", describe_code(&self.code).if_empty("code"), self.code.len(), &hex::encode(hash)[..16]);
                let want = match id { Some(s) => s, None => self.free_id()? };
                self.create(want, endowment)?
            }
        };
        self.provide(sid)?;
        // usable once the code is a preimage at the lookup anchor the next package names
        self.wait(&format!("service {sid}'s code at the lookup anchor"), |d| {
            let a = d.c.anchor()?; let l = d.c.lookup_anchor(a)?;
            Ok(d.c.rpc.service_preimage(&l.hash, sid, &hash)?.map(|_| ()))
        })?;
        let info = self.c.record(sid, None)?.ok_or_else(|| anyhow!("service {sid} vanished while deploying"))?;
        if let Some(p) = state {
            std::fs::write(p, serde_json::to_string_pretty(&json!({"service_id": sid, "code_hash": hex::encode(hash),
                "bootstrap_id": self.bootstrap, "created": info.created}))?)
                .with_context(|| format!("write {}", p.display()))?;
        }
        eprintln!("deploy: service {sid} ready (created #{}, balance {})", info.created, info.balance);
        Ok(sid)
    }

    fn free_id(&mut self) -> Result<u32> {
        let at = self.c.rpc.best_block()?.hash;
        let listed = self.c.rpc.list_services(&at)?;
        for s in 1..(1u32 << 16) {
            if !listed.contains(&s) && self.c.record(s, Some(at))?.is_none() { return Ok(s); }
        }
        bail!("no free service id below 2^16")
    }

    fn create(&mut self, sid: u32, endowment: u64) -> Result<u32> {
        let hash = blake2b256(&self.code);
        if let Some(i) = self.c.record(self.bootstrap, None)? {
            let at = self.c.rpc.best_block()?.hash;
            let who = self.c.rpc.service_preimage(&at, self.bootstrap, &i.code_hash)?.map(|b| describe_code(&b)).unwrap_or_default();
            eprintln!("deploy: Bootstrap service {}: {}", self.bootstrap, who.as_str().if_empty("unnamed code"));
        } else { bail!("no service {} (the Bootstrap service) on this chain", self.bootstrap); }
        let memo = self.c.param("transfer_memo_size").unwrap_or(128) as usize;
        let expiry = self.c.param("recent_block_count").unwrap_or(8) as u32 + 6;
        let send = |d: &mut Self| -> Result<Value> {
            let payload = create_service_payload(&hash, d.code.len() as u64, endowment, memo, Some(sid));
            let r = d.c.submit(d.bootstrap, &payload, &[], 0, &[], None)?;
            eprintln!("deploy: CreateService(id {sid}, {} bytes, endowment {endowment}): package {}.. on core {}, anchor #{}",
                d.code.len(), &r["package_hash"].as_str().unwrap()[..16], r["core"], r["anchor_slot"]);
            Ok(r)
        };
        let mut receipt = send(self)?;
        self.wait(&format!("service {sid} to be created"), |d| {
            let r = d.c.record(sid, None)?;
            if d.ours(&r) { eprintln!("deploy: service {sid} created"); return Ok(Some(sid)); }
            let st = d.c.status(&hex32(receipt["package_hash"].as_str().unwrap())?, &hex32(receipt["anchor"].as_str().unwrap())?)
                .unwrap_or(json!({"Unknown": null}));
            let kind = st.as_object().and_then(|o| o.keys().next().cloned()).unwrap_or_default();
            let head = d.c.rpc.best_block()?.slot;
            if kind == "Failed" || (kind == "Unknown" || st.is_null()) && head > receipt["anchor_slot"].as_u64().unwrap_or(0) as u32 + expiry {
                eprintln!("deploy: package {} ({st}); resubmitting", &receipt["package_hash"].as_str().unwrap()[..16]);
                receipt = send(d)?;
            }
            Ok(None)
        })
    }

    fn provide(&mut self, sid: u32) -> Result<()> {
        let (hash, n) = (blake2b256(&self.code), self.code.len() as u64);
        let mut sent_at: Option<u32> = None;
        self.wait(&format!("service {sid}'s code to be provided"), |d| {
            let at = d.c.rpc.best_block()?;
            if d.c.rpc.service_preimage(&at.hash, sid, &hash)?.is_some() { return Ok(Some(())); }
            let req = d.c.rpc.service_request(&at.hash, sid, &hash, n)?;
            if req.is_null() { bail!("service {sid} has not requested its code: nothing can provide it"); }
            if sent_at.is_none_or(|s| at.slot >= s + 3) {
                d.c.rpc.submit_preimage(sid, &d.code)?;
                eprintln!("deploy: submitPreimage({sid}, {n} bytes) at #{}", at.slot);
                sent_at = Some(at.slot);
            }
            Ok(None)
        })
    }
}

trait IfEmpty { fn if_empty(&self, d: &str) -> String; }
impl IfEmpty for str { fn if_empty(&self, d: &str) -> String { if self.is_empty() { d.into() } else { self.into() } } }
impl IfEmpty for String { fn if_empty(&self, d: &str) -> String { self.as_str().if_empty(d) } }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_service_layout() {
        let p = create_service_payload(&[9; 32], 1234, 1_000_000_000, 128, Some(5));
        // E(1) + variant + hash + 4 u64 + memo + registration none + deposit u64 + some(id)
        assert_eq!(p.len(), 1 + 1 + 32 + 32 + 128 + 1 + 8 + 5 + 32);
        assert_eq!(&p[..2], &[1, 0]);
        assert_eq!(&p[p.len() - 32 - 5..p.len() - 32], &[1, 5, 0, 0, 0]);
    }

    #[test]
    fn bytes_args() {
        assert_eq!(bytes_arg("0x0a0b", false).unwrap(), vec![10, 11]);
        assert_eq!(bytes_arg("root:note", false).unwrap(), b"root:note".to_vec());
        assert_eq!(bytes_arg("0a0b", true).unwrap(), vec![10, 11]);
    }
}
