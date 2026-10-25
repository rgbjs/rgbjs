//! Creating v0.12 contracts and invoices offline.
//!
//! Nothing here holds keys or touches the network. Seals are derived from the
//! caller's outpoint and a secret noise seed (random bytes the caller keeps),
//! the way rgb-wallet derives them from its wallet seed.

use std::str::FromStr;

use bp::seals::WTxoSeal;
use bp::Outpoint;
use commit_verify::{Digest, DigestExt, Sha256};
use hypersonic::{ContractId, Issuer};
use rgb::{Contract, CreateParams, RgbSealDef};
use serde_json::json;
use strict_encoding::{StreamWriter, StrictWriter};
use wasm_bindgen::prelude::*;

use crate::mem::{PileMem, StockMem};
use crate::{read_issuer, KNOWN_ISSUERS};

type Res<T> = Result<T, String>;

fn noise_engine(seed: &[u8]) -> Res<Sha256> {
    if seed.len() < 16 {
        return Err("the noise seed must be at least 16 random bytes".into());
    }
    let mut engine = Sha256::new();
    engine.input_raw(seed);
    Ok(engine)
}

fn issuers(extra: Option<Vec<u8>>) -> Res<Vec<Issuer>> {
    let mut list: Vec<Issuer> = KNOWN_ISSUERS
        .iter()
        .map(|b| read_issuer(b).map(|(c, s, _)| Issuer::new(c, s).expect("embedded issuer")))
        .collect::<Res<_>>()?;
    if let Some(bytes) = extra {
        let (c, s, _) = read_issuer(&bytes)?;
        list.push(Issuer::new(c, s).map_err(|e| e.to_string())?);
    }
    Ok(list)
}

/// Issues a v0.12 contract and returns its `RGBCNSGN` consignment file, the
/// same as `rgb backup` writes.
///
/// `params` is JSON in the shape of rgb-wallet's issue parameter file
/// (`consensus`, `testnet`, `issuer`, `name`, `method`, `timestamp`, `global`,
/// `owned`), with each owned `seal` an outpoint `txid:vout` you control.
/// `noise_seed` is secret random bytes that blind the seals; keep it to spend
/// them later. `issuer` is an optional `.issuer` file for an issuer rgbjs
/// doesn't already know.
#[wasm_bindgen(js_name = issueContract)]
pub fn issue_contract(params: &str, noise_seed: &[u8], issuer: Option<Vec<u8>>) -> Res<Vec<u8>> {
    let params: CreateParams<Outpoint> = serde_json::from_str(params).map_err(|e| format!("parameters: {e}"))?;
    if !params.testnet {
        return Err("mainnet is not supported: rgbjs creates on test networks only".into());
    }
    let issuer = issuers(issuer)?
        .into_iter()
        .find(|i| params.issuer.check(i.issuer_id()))
        .ok_or("no known issuer matches the `issuer` in the parameters; pass its .issuer file")?;
    let params = params.transform(noise_engine(noise_seed)?);
    let contract = Contract::<StockMem, PileMem>::issue(issuer, params, |_| Ok(())).map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    out.extend_from_slice(crate::CONSIGN_MAGIC);
    out.extend_from_slice(&rgb::CONSIGN_VERSION.to_be_bytes());
    let writer = StrictWriter::with(StreamWriter::new::<{ usize::MAX }>(&mut out));
    contract.export(writer).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Creates a v0.12 invoice paying to an auth token on one of your outputs.
///
/// The token is derived from `outpoint` (`txid:vout` you control), the secret
/// `noise_seed` and `nonce`, as rgb-wallet's `invoice` does. `value` is the
/// amount (or, for non-fungible state, a string). Returns JSON with the
/// `invoice`, the `authToken`, and the `seal` details you must keep.
#[wasm_bindgen(js_name = createInvoice)]
pub fn create_invoice(contract_id: &str, value: &str, outpoint: &str, noise_seed: &[u8], nonce: u64) -> Res<String> {
    let id = ContractId::from_str(contract_id).map_err(|e| format!("contract ID: {e}"))?;
    let outpoint = Outpoint::from_str(outpoint).map_err(|e| format!("outpoint: {e}"))?;
    if value.is_empty() || value.contains(['/', '@', '?', '#']) {
        return Err("the value must be a number or a plain string".into());
    }
    let seal = WTxoSeal::no_fallback(outpoint, noise_engine(noise_seed)?, nonce);
    let auth = seal.auth_token();
    // The single-prefix form the upstream tests treat as canonical; rgb-wallet
    // 0.12.0-rc.3 prints `contract:tb@contract:<id>` but reads both.
    let invoice = format!("contract:tb@{id:-}/{value}@{auth}/");
    crate::parse_invoice(&invoice)?;
    Ok(json!({
        "line": "v0.12",
        "invoice": invoice,
        "authToken": auth.to_string(),
        "seal": { "outpoint": outpoint.to_string(), "nonce": nonce },
    })
    .to_string())
}
