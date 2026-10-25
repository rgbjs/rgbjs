//! wasm bindings for RGB v0.12 (RGB-WG / LNP/BP line).
//!
//! Everything here is offline: no keys, no network. Results are JSON strings,
//! parsed on the JavaScript side.

use std::collections::HashMap;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_invoice() {
        let s = "contract:tb@contract:nztjpjRX-_NVDDgd-EHENsqL-AKsGsyV-F6LzljB-X4WTv0Y/10@at:FWKMBVDc-NBk8dAZA-tYGt2HIP-MtzSRO17-8g3QvRIN-nuPjug/";
        println!("{}", parse_invoice(s).unwrap());
        println!("{}", parse_contract_id("contract:nztjpjRX-_NVDDgd-EHENsqL-AKsGsyV-F6LzljB-X4WTv0Y").unwrap());
    }
}
