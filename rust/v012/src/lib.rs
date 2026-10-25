//! wasm bindings for RGB v0.12 (RGB-WG / LNP/BP line).
//!
//! Everything here is offline: no keys, no network. Results are JSON strings,
//! parsed on the JavaScript side.

#[macro_use]
extern crate amplify;

mod create;
mod mem;

use std::collections::HashMap;
use std::convert::Infallible;
use std::str::FromStr;

use hypersonic::{Consensus, ContractId};
use rgb_invoice::{RgbBeneficiary, RgbInvoice};
use serde_json::json;
use sonic_callreq::CallScope;
use strict_types::value::StrictNum;
use strict_types::StrictVal;
use wasm_bindgen::prelude::*;

fn consensus(c: Consensus) -> &'static str {
    match c {
        Consensus::None => "none",
        Consensus::Bitcoin => "bitcoin",
        Consensus::Liquid => "liquid",
        Consensus::Prime => "prime",
    }
}

/// Parses a v0.12 invoice (`contract:...`) into JSON.
#[wasm_bindgen(js_name = parseInvoice)]
pub fn parse_invoice(s: &str) -> Result<String, String> {
    let inv = <RgbInvoice>::from_str(s).map_err(|e| e.to_string())?;
    let (contract_id, contract_query) = match &inv.scope {
        CallScope::ContractId(id) => (Some(id.to_string()), None),
        CallScope::ContractQuery(q) => (None, Some(q.to_string())),
    };
    let beneficiary = match &inv.auth {
        RgbBeneficiary::Token(t) => json!({"type": "auth-token", "value": t.to_string()}),
        RgbBeneficiary::WitnessOut(w) => json!({"type": "witness-out", "value": w.to_string()}),
    };
    let amount = match &inv.data {
        Some(StrictVal::Number(StrictNum::Uint(n))) => Some(n.to_string()),
        _ => None,
    };
    Ok(json!({
        "line": "v0.12",
        "invoice": inv.to_string(),
        "network": consensus(inv.layer1.consensus),
        "testnet": inv.layer1.testnet,
        "contractId": contract_id,
        "contractQuery": contract_query,
        "api": inv.api.as_ref().map(|a| a.to_string()),
        "method": inv.call.as_ref().map(|c| c.method.to_string()),
        "stateName": inv.call.as_ref().and_then(|c| c.owned.as_ref()).map(|o| o.to_string()),
        "amount": amount,
        "data": inv.data.as_ref().map(|d| d.to_string()),
        "beneficiary": beneficiary,
        "expiry": inv.expiry.map(|e| e.timestamp()),
        "endpoints": inv.endpoints.iter().map(|e| e.to_string()).collect::<Vec<_>>(),
        "unknownQuery": inv.unknown_query.iter().collect::<HashMap<_, _>>(),
    })
    .to_string())
}

/// Parses a v0.12 contract ID (`contract:...`) and returns it in canonical form.
#[wasm_bindgen(js_name = parseContractId)]
pub fn parse_contract_id(s: &str) -> Result<String, String> {
    let id = ContractId::from_str(s).map_err(|e| e.to_string())?;
    let hex: String = id.to_byte_array().iter().map(|b| format!("{b:02x}")).collect();
    Ok(json!({"line": "v0.12", "contractId": id.to_string(), "hex": hex}).to_string())
}

pub(crate) const CONSIGN_MAGIC: &[u8; 8] = b"RGBCNSGN";
const ISSUER_MAGIC: &[u8; 8] = b"ISSUER  ";

/// Issuers whose semantics (APIs, types, libraries) rgbjs trusts out of the box:
/// RGB20 FNA from RGB-WG/rgb v0.12.0-rc.3 `examples/`, and RGB21 UniqueNFT and
/// RGB25 UniquelyFungible from pandora-prime/rgb-issuers at 49b4048 (all by
/// the same developer, dns:pandoraprime.ch; Apache-2.0).
pub(crate) const KNOWN_ISSUERS: &[&[u8]] = &[
    include_bytes!("../issuers/RGB20-FNA.issuer"),
    include_bytes!("../issuers/RGB21-UniqueNFT-v0-FtU80Q.issuer"),
    include_bytes!("../issuers/RGB25-UniquelyFungible-v0-aiRB3w.issuer"),
];

fn issuer_id(codex: &hypersonic::Codex, semantics: &hypersonic::Semantics) -> hypersonic::IssuerId {
    hypersonic::IssuerId {
        codex_id: codex.codex_id(),
        version: semantics.version,
        checksum: semantics.apis_checksum(),
    }
}

fn known_issuers_full() -> &'static [(hypersonic::IssuerId, String)] {
    static KNOWN: std::sync::OnceLock<Vec<(hypersonic::IssuerId, String)>> = std::sync::OnceLock::new();
    KNOWN.get_or_init(|| {
        KNOWN_ISSUERS
            .iter()
            .map(|b| read_issuer(b).expect("embedded issuer"))
            .map(|(c, s, _)| (issuer_id(&c, &s), format!("{s:?}")))
            .collect()
    })
}

/// Reads an `.issuer` file. Signatures are not checked.
pub(crate) fn read_issuer(bytes: &[u8]) -> Result<(hypersonic::Codex, hypersonic::Semantics, bool), String> {
    use strict_encoding::{StreamReader, StrictDecode, StrictReader};

    if bytes.len() < 10 || &bytes[..8] != ISSUER_MAGIC {
        return Err("not an RGB v0.12 issuer file (missing ISSUER header)".into());
    }
    let mut reader = StrictReader::with(StreamReader::new::<{ usize::MAX }>(&bytes[10..]));
    let codex = hypersonic::Codex::strict_decode(&mut reader).map_err(|e| e.to_string())?;
    let semantics = hypersonic::Semantics::strict_decode(&mut reader).map_err(|e| e.to_string())?;
    semantics.check(&codex).map_err(|e| e.to_string())?;
    let sig = Option::<hypersonic::SigBlob>::strict_decode(&mut reader).map_err(|e| e.to_string())?;
    Ok((codex, semantics, sig.is_some()))
}

fn issuer_json(codex: &hypersonic::Codex, semantics: &hypersonic::Semantics) -> serde_json::Value {
    json!({
        "id": issuer_id(codex, semantics).to_string(),
        "codexId": codex.codex_id().to_string(),
        "name": codex.name.to_string(),
        "developer": codex.developer.to_string(),
        "version": semantics.version,
    })
}

/// Parses an `.issuer` file and returns its ID, to compare with the `issuer.id`
/// that `validateConsignment` reports.
#[wasm_bindgen(js_name = parseIssuer)]
pub fn parse_issuer(bytes: &[u8]) -> Result<String, String> {
    let (codex, semantics, signed) = read_issuer(bytes)?;
    let mut v = issuer_json(&codex, &semantics);
    v["signed"] = json!(signed);
    Ok(v.to_string())
}

/// Lists the issuers `validateConsignment` reports as trusted.
#[wasm_bindgen(js_name = knownIssuers)]
pub fn known_issuers() -> String {
    let list: Vec<_> = KNOWN_ISSUERS
        .iter()
        .map(|b| read_issuer(b).expect("embedded issuer"))
        .map(|(c, s, _)| issuer_json(&c, &s))
        .collect();
    json!(list).to_string()
}

/// Validates a v0.12 consignment file (`RGBCNSGN`, as written by `rgb backup`,
/// `rgb consign` or `rgb pay`) offline, by evaluating every operation into an
/// in-memory contract, as `rgb accept -u` does. Witness transactions come
/// from the consignment and are not looked up on chain. Issuer signatures
/// are not checked; `signed` says whether one is present.
#[wasm_bindgen(js_name = validateConsignment)]
pub fn validate_consignment(bytes: &[u8]) -> Result<String, String> {
    use bp::seals::TxoSeal;
    use rgb::{parse_consignment, Consignment, Contract};
    use strict_encoding::{StreamReader, StrictDecode, StrictReader};

    if bytes.len() < 10 || &bytes[..8] != CONSIGN_MAGIC {
        return Err("not an RGB v0.12 consignment file (missing RGBCNSGN header)".into());
    }
    let version = u16::from_be_bytes([bytes[8], bytes[9]]);
    if version != rgb::CONSIGN_VERSION {
        return Err(format!("unsupported consignment file version {version}"));
    }
    let mut reader = StrictReader::with(StreamReader::new::<{ usize::MAX }>(&bytes[10..]));
    let header_id = parse_consignment(&mut reader).map_err(|e| e.to_string())?;
    let consignment = Consignment::<TxoSeal>::strict_decode(&mut reader).map_err(|e| e.to_string())?;

    let mut signed = false;
    let articles = consignment
        .articles(|_, _, _| -> Result<_, Infallible> {
            signed = true;
            Ok(())
        })
        .map_err(|e| e.to_string())?;
    let contract_id = articles.contract_id();
    let issue = articles.issue();
    let codex = articles.codex();
    let id = issuer_id(codex, articles.semantics());
    // The issuer ID's checksum covers type IDs but not each type's contents
    // (field and variant names), so also require the semantics to match the
    // trusted issuer in full.
    let full = format!("{:?}", articles.semantics());
    let trusted = known_issuers_full().iter().any(|(kid, kfull)| *kid == id && *kfull == full);
    let mut issuer = issuer_json(codex, articles.semantics());
    issuer["trusted"] = json!(trusted);
    let mut out = json!({
        "line": "v0.12",
        "contractId": contract_id.to_string(),
        "name": issue.meta.name.to_string(),
        "network": consensus(issue.meta.consensus),
        "testnet": issue.meta.testnet,
        "issuer": issuer,
        "signed": signed,
    });
    let obj = out.as_object_mut().unwrap();
    // Neither is covered by the contract ID, and upstream checks neither.
    let mismatch = if header_id != contract_id {
        Some(format!("header names contract {header_id} but the articles are for {contract_id}"))
    } else if issue.genesis.codex_id != codex.codex_id() {
        Some(format!("genesis names codex {} but the contract uses {}", issue.genesis.codex_id, codex.codex_id()))
    } else {
        None
    };
    if let Some(error) = mismatch {
        obj.insert("valid".into(), json!(false));
        obj.insert("error".into(), json!(error));
        return Ok(out.to_string());
    }
    match Contract::<mem::StockMem, mem::PileMem>::with(articles, consignment, ()) {
        Ok(contract) => {
            let witnesses: Vec<_> = contract
                .witnesses()
                .map(|w| json!({"txid": w.id.to_string(), "status": w.status.to_string()}))
                .collect();
            let state = contract.state();
            let global: serde_json::Map<_, _> = state
                .immutable
                .iter()
                .map(|(name, atoms)| {
                    (name.to_string(), json!(atoms.iter().map(|a| a.data.verified.to_string()).collect::<Vec<_>>()))
                })
                .collect();
            // Unverified parts of global state (e.g. an NFT's name and image),
            // which the codex doesn't check: shown apart so they aren't mistaken
            // for verified state.
            let unverified: serde_json::Map<_, _> = state
                .immutable
                .iter()
                .filter_map(|(name, atoms)| {
                    let vals: Vec<_> = atoms.iter().filter_map(|a| a.data.unverified.as_ref().map(|u| u.to_string())).collect();
                    (!vals.is_empty()).then(|| (name.to_string(), json!(vals)))
                })
                .collect();
            let owned: serde_json::Map<_, _> = state
                .owned
                .iter()
                .map(|(name, cells)| {
                    let cells: Vec<_> = cells
                        .iter()
                        .map(|c| json!({
                            "outpoint": c.assignment.seal.primary.to_string(),
                            "value": c.assignment.data.to_string(),
                            "opid": c.addr.opid.to_string(),
                            "pos": c.addr.pos,
                        }))
                        .collect();
                    (name.to_string(), json!(cells))
                })
                .collect();
            let aggregated: serde_json::Map<_, _> =
                state.aggregated.iter().map(|(name, v)| (name.to_string(), json!(v.to_string()))).collect();
            obj.insert("valid".into(), json!(true));
            obj.insert("operations".into(), json!(contract.operations().count()));
            obj.insert("state".into(), json!({"global": global, "unverified": unverified, "owned": owned, "aggregated": aggregated}));
            obj.insert("witnesses".into(), json!(witnesses));
        }
        Err(err) => {
            obj.insert("valid".into(), json!(false));
            obj.insert("error".into(), json!(err.to_string()));
        }
    }
    Ok(out.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_invoice() {
        let s = "contract:tb@contract:nztjpjRX-_NVDDgd-EHENsqL-AKsGsyV-F6LzljB-X4WTv0Y/10@at:FWKMBVDc-NBk8dAZA-tYGt2HIP-MtzSRO17-8g3QvRIN-nuPjug/";
        println!("{}", parse_invoice(s).unwrap());
        println!("{}", parse_contract_id("contract:nztjpjRX-_NVDDgd-EHENsqL-AKsGsyV-F6LzljB-X4WTv0Y").unwrap());
    }

    #[test]
    fn demo_token() {
        let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/vectors/v012/DemoToken.rgb")).unwrap();
        println!("{}", validate_consignment(&bytes).unwrap());
    }
}







