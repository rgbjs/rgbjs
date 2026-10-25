//! wasm bindings for RGB v0.11 (rgb-protocol / RGB-Tools line).
//!
//! Everything here is offline: no keys, no network. Results are JSON strings,
//! parsed on the JavaScript side.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::OnceLock;

use rgbinvoice::{Beneficiary, InvoiceState, RgbInvoice, RgbTransport};
use rgbstd::containers::{
    ConsignmentExt, Contract, FileContent, Transfer, ValidConsignment,
};
use rgbstd::validation::{
    ResolveWitness, ValidationConfig, ValidationError, WitnessResolverError, WitnessStatus,
};
use rgbstd::contract::IssuerWrapper;
use rgbstd::vm::WitnessOrd;
use rgbstd::{SchemaId, TypeSystem};
use schemata::{
    CollectibleFungibleAsset, InflatableFungibleAsset, NonInflatableAsset, PermissionedFungibleAsset,
    UniqueDigitalAsset,
};
use rgbstd::{ChainNet, ContractId, Txid};
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

fn network(cn: ChainNet) -> &'static str {
    match cn {
        ChainNet::BitcoinMainnet => "bitcoin",
        ChainNet::BitcoinTestnet3 => "testnet3",
        ChainNet::BitcoinTestnet4 => "testnet4",
        ChainNet::BitcoinSignet => "signet",
        ChainNet::BitcoinSignetCustom => "signet-custom",
        ChainNet::BitcoinRegtest => "regtest",
        ChainNet::LiquidMainnet => "liquid",
        ChainNet::LiquidTestnet => "liquid-testnet",
    }
}

fn transport(t: &RgbTransport) -> Value {
    match t {
        RgbTransport::JsonRpc { tls, host } => json!({"type": "json-rpc", "tls": tls, "host": host}),
        RgbTransport::RestHttp { tls, host } => json!({"type": "rest", "tls": tls, "host": host}),
        RgbTransport::WebSockets { tls, host } => json!({"type": "websockets", "tls": tls, "host": host}),
        RgbTransport::Storm {} => json!({"type": "storm"}),
        _ => json!({"type": "unknown"}),
    }
}

/// Parses a v0.11 invoice (`rgb:...`) into JSON.
#[wasm_bindgen(js_name = parseInvoice)]
pub fn parse_invoice(s: &str) -> Result<String, String> {
    let inv = RgbInvoice::from_str(s).map_err(|e| e.to_string())?;
    let chain_net = inv.chain_network();
    let kind = match inv.beneficiary.clone().into_inner() {
        Beneficiary::BlindedSeal(_) => "blinded-utxo",
        Beneficiary::WitnessVout(..) => "witness-vout",
    };
    let beneficiary = json!({"type": kind, "value": inv.beneficiary.to_string()});
    let (amount, data) = match &inv.assignment_state {
        Some(InvoiceState::Amount(a)) => (Some(a.value().to_string()), None),
        Some(InvoiceState::Data(d)) => (None, Some(d.to_string())),
        _ => (None, None),
    };
    Ok(json!({
        "line": "v0.11",
        "invoice": inv.to_string(),
        "network": network(chain_net),
        "testnet": !matches!(chain_net, ChainNet::BitcoinMainnet | ChainNet::LiquidMainnet),
        "contractId": inv.contract.map(|c| c.to_string()),
        "schemaId": inv.schema.map(|s| s.to_string()),
        "assignmentName": inv.assignment_name.as_ref().map(|n| n.to_string()),
        "amount": amount,
        "data": data,
        "beneficiary": beneficiary,
        "expiry": inv.expiry,
        "transports": inv.transports.iter().map(transport).collect::<Vec<_>>(),
        "unknownQuery": inv.unknown_query.iter().collect::<HashMap<_, _>>(),
    })
    .to_string())
}

/// Parses a v0.11 contract ID (`rgb:...`) and returns it in canonical form.
#[wasm_bindgen(js_name = parseContractId)]
pub fn parse_contract_id(s: &str) -> Result<String, String> {
    let id = ContractId::from_str(s).map_err(|e| e.to_string())?;
    Ok(json!({"line": "v0.11", "contractId": id.to_string(), "hex": id.to_byte_array().iter().map(|b| format!("{b:02x}")).collect::<String>()}).to_string())
}

/// The standard v0.11 schemas from `rgb-schemas`, whose type systems are
/// trusted when validating. A consignment for any other schema is rejected.
struct KnownSchema {
    name: &'static str,
    id: SchemaId,
    types: TypeSystem,
    /// `Debug` of each type, names included, for the strict comparison in [`trusted`].
    full: HashMap<String, String>,
}

fn known() -> &'static [KnownSchema] {
    static KNOWN: OnceLock<Vec<KnownSchema>> = OnceLock::new();
    fn entry<W: IssuerWrapper>(name: &'static str) -> KnownSchema {
        let types = W::types();
        let full = types.iter().map(|(id, ty)| (id.to_string(), format!("{ty:?}"))).collect();
        KnownSchema { name, id: W::schema().schema_id(), types, full }
    }
    KNOWN.get_or_init(|| {
        vec![
            entry::<NonInflatableAsset>("NIA"),
            entry::<CollectibleFungibleAsset>("CFA"),
            entry::<UniqueDigitalAsset>("UDA"),
            entry::<InflatableFungibleAsset>("IFA"),
            entry::<PermissionedFungibleAsset>("PFA"),
        ]
    })
}

/// Lists the schemas `validateConsignment` accepts.
#[wasm_bindgen(js_name = knownSchemas)]
pub fn known_schemas() -> String {
    json!(known().iter().map(|k| json!({"name": k.name, "schemaId": k.id.to_string()})).collect::<Vec<_>>())
        .to_string()
}

fn trusted(id: SchemaId, types: &TypeSystem) -> Result<(&'static str, TypeSystem), String> {
    let k = known().iter().find(|k| k.id == id).ok_or_else(|| {
        format!("unknown schema {id}; only the standard rgb-schemas 0.11.1 schemas are supported")
    })?;
    // rgb-consensus compares types with `Ty`'s PartialEq, which ignores field
    // and variant names. Names change how state reads, so require the full
    // definition to match.
    for (sem_id, ty) in types.iter() {
        if k.full.get(&sem_id.to_string()) != Some(&format!("{ty:?}")) {
            return Err(format!("type {sem_id} in the consignment differs from the trusted {} definition", k.name));
        }
    }
    Ok((k.name, k.types.clone()))
}

/// Resolves witness transactions only from the transactions carried in the
/// consignment itself, as `Tentative`. Nothing is looked up on chain.
struct OfflineResolver(HashMap<Txid, rgbstd::bitcoin::Transaction>);

impl ResolveWitness for OfflineResolver {
    fn resolve_witness(&self, witness_id: Txid) -> Result<WitnessStatus, WitnessResolverError> {
        Ok(match self.0.get(&witness_id) {
            Some(tx) => WitnessStatus::Resolved(tx.clone(), WitnessOrd::Tentative),
            None => WitnessStatus::Unresolved,
        })
    }

    fn check_chain_net(&self, _: ChainNet) -> Result<(), WitnessResolverError> { Ok(()) }
}

fn report<const T: bool>(
    kind: &str,
    contract_id: ContractId,
    schema_id: String,
    schema_name: &str,
    chain_net: ChainNet,
    witnesses: Vec<(Txid, bool)>,
    terminals: Value,
    resolver: &OfflineResolver,
    res: Result<ValidConsignment<T>, ValidationError>,
) -> String
where
    ValidConsignment<T>: Import,
{
    let witnesses: Vec<_> = witnesses
        .into_iter()
        .map(|(txid, included)| json!({"txid": txid.to_string(), "included": included}))
        .collect();
    let base = json!({
        "line": "v0.11",
        "kind": kind,
        "contractId": contract_id.to_string(),
        "schemaId": schema_id,
        "schema": schema_name,
        "network": network(chain_net),
        "witnesses": witnesses,
        "terminals": terminals,
    });
    let mut out = base.as_object().unwrap().clone();
    match res {
        Ok(valid) => {
            let status = valid.validation_status();
            out.insert("valid".into(), json!(true));
            out.insert("warnings".into(), json!(status.warnings.iter().map(|w| w.to_string()).collect::<Vec<_>>()));
            out.insert("info".into(), json!(status.info.iter().map(|i| i.to_string()).collect::<Vec<_>>()));
            match contract_state(valid, resolver) {
                Ok(state) => out.insert("state".into(), state),
                Err(e) => out.insert("stateError".into(), json!(e)),
            };
        }
        Err(ValidationError::InvalidConsignment(failure)) => {
            out.insert("valid".into(), json!(false));
            out.insert("error".into(), json!(failure.to_string()));
        }
        Err(err) => {
            out.insert("valid".into(), json!(false));
            out.insert("error".into(), json!(err.to_string()));
        }
    }
    Value::Object(out).to_string()
}

/// Imports a validated consignment into an in-memory stock and reads its
/// state back, as `rgb state` does: global state, and every allocation with
/// its seal (the seals the consignment reveals; blinded ones stay hidden).
trait Import {
    fn import(self, stock: &mut rgbstd::persistence::Stock, resolver: &OfflineResolver) -> Result<(), String>;
}

impl Import for ValidConsignment<false> {
    fn import(self, stock: &mut rgbstd::persistence::Stock, resolver: &OfflineResolver) -> Result<(), String> {
        stock.import_contract(self, resolver).map_err(|e| e.to_string())
    }
}

impl Import for ValidConsignment<true> {
    fn import(self, stock: &mut rgbstd::persistence::Stock, resolver: &OfflineResolver) -> Result<(), String> {
        stock.accept_transfer(self, resolver).map_err(|e| e.to_string())
    }
}

fn contract_state<const T: bool>(valid: ValidConsignment<T>, resolver: &OfflineResolver) -> Result<Value, String>
where ValidConsignment<T>: Import {
    use rgbstd::contract::FilterIncludeAll;
    use rgbstd::persistence::Stock;

    let id = valid.contract_id();
    // Outpoints spent by the witness transactions the consignment carries.
    let spent: std::collections::HashSet<String> = resolver
        .0
        .values()
        .flat_map(|tx| tx.input.iter().map(|i| i.previous_output.to_string()))
        .collect();
    let mut stock = Stock::in_memory();
    valid.import(&mut stock, resolver)?;
    let data = stock.contract_data(id).map_err(|e| e.to_string())?;
    let global: serde_json::Map<_, _> = data
        .schema
        .global_types
        .values()
        .map(|g| (g.name.to_string(), json!(data.global(g.name.clone()).map(|v| v.to_string()).collect::<Vec<_>>())))
        .collect();
    let mut owned = serde_json::Map::new();
    for d in data.schema.owned_types.values() {
        let name = d.name.clone();
        let mut cells = vec![];
        let witness = |w: Option<Txid>| w.map(|w| w.to_string());
        let is_spent = |seal: &dyn std::fmt::Display| spent.contains(&seal.to_string());
        if let Ok(a) = data.fungible(name.clone(), FilterIncludeAll) {
            cells.extend(a.map(|a| json!({"seal": a.seal.to_string(), "value": a.state.value().to_string(), "witness": witness(a.witness), "spent": is_spent(&a.seal)})));
        }
        if let Ok(a) = data.data(name.clone(), FilterIncludeAll) {
            // UDA-style data is a strict-encoded Allocation: show it as fraction@index.
            let value = |d: &rgbstd::RevealedData| {
                use strict_encoding::StrictDeserialize;
                rgbinvoice::Allocation::from_strict_serialized(d.clone().into())
                    .map(|a| a.to_string())
                    .unwrap_or_else(|_| d.to_string())
            };
            cells.extend(a.map(|a| json!({"seal": a.seal.to_string(), "value": value(&a.state), "witness": witness(a.witness), "spent": is_spent(&a.seal)})));
        }
        if let Ok(a) = data.rights(name.clone(), FilterIncludeAll) {
            cells.extend(a.map(|a| json!({"seal": a.seal.to_string(), "value": "right", "witness": witness(a.witness), "spent": is_spent(&a.seal)})));
        }
        owned.insert(name.to_string(), json!(cells));
    }
    Ok(json!({"global": global, "owned": owned}))
}

/// Validates a v0.11 consignment file (the `RGB\0TFR` transfer or `RGB\0CON`
/// contract binary) offline. Witness transactions are taken from the
/// consignment; whether they are mined is NOT checked, so `valid: true` means
/// "valid if the listed witnesses confirm".
#[wasm_bindgen(js_name = validateConsignment)]
pub fn validate_consignment(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() < 7 || &bytes[..4] != b"RGB\0" {
        return Err("not an RGB v0.11 consignment file (missing RGB\\0 header)".into());
    }
    match &bytes[4..7] {
        b"TFR" => {
            let c = Transfer::load(bytes).map_err(|e| e.to_string())?;
            let chain_net = c.genesis.chain_net;
            let txes: HashMap<_, _> = c
                .bundles
                .iter()
                .filter_map(|bw| bw.pub_witness.tx().cloned())
                .map(|tx| (tx.compute_txid(), tx))
                .collect();
            let witnesses = c
                .bundles
                .iter()
                .map(|bw| (bw.pub_witness.txid(), bw.pub_witness.tx().is_some()))
                .collect();
            let (id, schema) = (c.contract_id(), c.schema_id().to_string());
            // The blinded seals the sender says the transfer is for. They are
            // hints, not validated: a recipient should find its own here.
            let terminals: Vec<_> = c
                .terminals
                .iter()
                .map(|(bundle, seals)| {
                    json!({"bundleId": bundle.to_string(), "seals": seals.iter().map(|s| s.to_string()).collect::<Vec<_>>()})
                })
                .collect();
            let (name, trusted_typesystem) = trusted(c.schema_id(), &c.types)?;
            let config = ValidationConfig { chain_net, trusted_typesystem, ..Default::default() };
            let resolver = OfflineResolver(txes);
            let res = c.validate(&resolver, &config);
            Ok(report("transfer", id, schema, name, chain_net, witnesses, json!(terminals), &resolver, res))
        }
        b"CON" => {
            let c = Contract::load(bytes).map_err(|e| e.to_string())?;
            let chain_net = c.genesis.chain_net;
            let (id, schema) = (c.contract_id(), c.schema_id().to_string());
            let (name, trusted_typesystem) = trusted(c.schema_id(), &c.types)?;
            let config = ValidationConfig { chain_net, trusted_typesystem, ..Default::default() };
            let resolver = OfflineResolver(HashMap::new());
            let res = c.validate(&resolver, &config);
            Ok(report("contract", id, schema, name, chain_net, vec![], json!([]), &resolver, res))
        }
        other => Err(format!(
            "unsupported v0.11 file type {:?}; expected TFR or CON",
            String::from_utf8_lossy(other)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_contract() {
        let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/vectors/v011/demo.contract.rgb")).unwrap();
        println!("{}", validate_consignment(&bytes).unwrap());
    }
}

