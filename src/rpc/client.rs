//! JIP-2 over a WebSocket: JSON-RPC 2.0, hashes and blobs as base64. A read that fails on the
//! transport is retried once on a fresh connection; a submission is never retried (it may
//! have been delivered).

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde_json::{Value, json};
use std::net::TcpStream;
use tungstenite::{Message, WebSocket, stream::MaybeTlsStream};

pub fn b64(b: &[u8]) -> Value {
    Value::String(B64.encode(b))
}

pub fn unb64(v: &Value) -> Result<Vec<u8>> {
    let s = v.as_str().ok_or_else(|| anyhow!("expected a base64 string, got {v}"))?;
    Ok(B64.decode(s)?)
}

pub fn hash32(v: &Value) -> Result<[u8; 32]> {
    unb64(v)?.try_into().map_err(|_| anyhow!("expected a 32-byte hash"))
}

/// A block as JIP-2 describes it.
#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub hash: [u8; 32],
    pub slot: u32,
}

impl Block {
    fn from(v: &Value) -> Result<Self> {
        Ok(Self {
            hash: hash32(&v["header_hash"]).context("block descriptor")?,
            slot: v["slot"].as_u64().ok_or_else(|| anyhow!("block descriptor: no slot in {v}"))? as u32,
        })
    }
}

#[derive(Debug)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JIP-2 error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for RpcError {}

pub struct Client {
    url: String,
    ws: Option<WebSocket<MaybeTlsStream<TcpStream>>>,
    next_id: u64,
}

impl Client {
    pub fn new(url: &str) -> Self {
        Self { url: url.to_string(), ws: None, next_id: 0 }
    }

    fn once(&mut self, method: &str, params: &Value) -> Result<Value> {
        if self.ws.is_none() {
            let (ws, _) = tungstenite::connect(self.url.as_str()).with_context(|| format!("connect {}", self.url))?;
            self.ws = Some(ws);
        }
        let ws = self.ws.as_mut().unwrap();
        self.next_id += 1;
        let id = self.next_id;
        ws.send(Message::text(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()))?;
        loop {
            let msg = match ws.read()? {
                Message::Text(t) => serde_json::from_str::<Value>(t.as_str())?,
                Message::Binary(b) => serde_json::from_slice::<Value>(&b)?,
                Message::Close(_) => bail!("the node closed the connection"),
                _ => continue,
            };
            // ours: our id, or null on an error the node could not tie to a request
            match msg.get("id") {
                Some(Value::Number(n)) if n.as_u64() == Some(id) => return Ok(msg),
                Some(Value::Null) => return Ok(msg),
                _ => continue,
            }
        }
    }

    pub fn call(&mut self, method: &str, params: Value, retry: bool) -> Result<Value> {
        let msg = match self.once(method, &params) {
            Ok(m) => m,
            Err(e) if retry => {
                self.ws = None;
                self.once(method, &params).with_context(|| format!("{method} (retried after: {e})"))?
            }
            Err(e) => {
                self.ws = None;
                return Err(e);
            }
        };
        if let Some(err) = msg.get("error").filter(|e| !e.is_null()) {
            return Err(RpcError {
                code: err["code"].as_i64().unwrap_or(0),
                message: err["message"].as_str().unwrap_or("").to_string(),
            }
            .into());
        }
        Ok(msg.get("result").cloned().unwrap_or(Value::Null))
    }

    // ---- the JIP-2 methods ------------------------------------------------------------
    pub fn parameters(&mut self) -> Result<Value> {
        let p = self.call("parameters", json!([]), true)?;
        p.get("V1").cloned().ok_or_else(|| anyhow!("parameters: no V1 member in {p}"))
    }
    pub fn best_block(&mut self) -> Result<Block> {
        Block::from(&self.call("bestBlock", json!([]), true)?)
    }
    pub fn finalized_block(&mut self) -> Result<Block> {
        Block::from(&self.call("finalizedBlock", json!([]), true)?)
    }
    /// The block's parent, or the block itself if it has none (the genesis block).
    pub fn parent(&mut self, b: Block) -> Result<Block> {
        match self.call("parent", json!([b64(&b.hash)]), true) {
            Ok(v) => Block::from(&v),
            Err(e) if e.downcast_ref::<RpcError>().is_some() => Ok(b),
            Err(e) => Err(e),
        }
    }
    pub fn state_root(&mut self, at: &[u8; 32]) -> Result<[u8; 32]> {
        hash32(&self.call("stateRoot", json!([b64(at)]), true)?)
    }
    pub fn beefy_root(&mut self, at: &[u8; 32]) -> Result<[u8; 32]> {
        hash32(&self.call("beefyRoot", json!([b64(at)]), true)?)
    }
    pub fn service_data(&mut self, at: &[u8; 32], service: u32) -> Result<Option<Vec<u8>>> {
        let v = self.call("serviceData", json!([b64(at), service]), true)?;
        if v.is_null() { Ok(None) } else { Ok(Some(unb64(&v)?)) }
    }
    pub fn service_value(&mut self, at: &[u8; 32], service: u32, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let v = self.call("serviceValue", json!([b64(at), service, b64(key)]), true)?;
        if v.is_null() { Ok(None) } else { Ok(Some(unb64(&v)?)) }
    }
    pub fn service_preimage(&mut self, at: &[u8; 32], service: u32, hash: &[u8; 32]) -> Result<Option<Vec<u8>>> {
        let v = self.call("servicePreimage", json!([b64(at), service, b64(hash)]), true)?;
        if v.is_null() { Ok(None) } else { Ok(Some(unb64(&v)?)) }
    }
    pub fn service_request(&mut self, at: &[u8; 32], service: u32, hash: &[u8; 32], len: u64) -> Result<Value> {
        self.call("serviceRequest", json!([b64(at), service, b64(hash), len]), true)
    }
    pub fn list_services(&mut self, at: &[u8; 32]) -> Result<Vec<u32>> {
        let v = self.call("listServices", json!([b64(at)]), true)?;
        v.as_array().map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as u32)).collect())
            .ok_or_else(|| anyhow!("listServices: expected a list, got {v}"))
    }
    pub fn work_package_status(&mut self, at: &[u8; 32], package: &[u8; 32], anchor: &[u8; 32]) -> Result<Value> {
        self.call("workPackageStatus", json!([b64(at), b64(package), b64(anchor)]), true)
    }
    /// JIP-2 submitWorkPackage: the encoded package, and the extrinsic data its items reference.
    pub fn submit_work_package(&mut self, core: u32, package: &[u8], extrinsics: &[Vec<u8>]) -> Result<Value> {
        let xs: Vec<Value> = extrinsics.iter().map(|x| b64(x)).collect();
        self.call("submitWorkPackage", json!([core, b64(package), xs]), false)
    }
    pub fn submit_preimage(&mut self, service: u32, preimage: &[u8]) -> Result<Value> {
        self.call("submitPreimage", json!([service, b64(preimage)]), false)
    }
}
