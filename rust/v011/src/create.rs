//! Creating v0.11 contracts and invoices offline, from JSON parameters.
//!
//! Nothing here holds keys or touches the network. The caller supplies the
//! secret blinding factors (random u64s), so the same input always gives the
//! same output and the caller decides where secrets are kept.

use std::str::FromStr;

use amplify::confinement::SmallBlob;
use amplify::Wrapper;
use rgbinvoice::{Allocation, Beneficiary, RgbInvoiceBuilder, XChainNet};
use rgbstd::containers::{ConsignmentExt, FileContent};
use rgbstd::contract::IssuerWrapper;
use rgbstd::invoice::Precision;
use rgbstd::persistence::Stock;
use rgbstd::stl::{
    AssetSpec, ContractTerms, EmbeddedMedia, MediaType, Name, RejectListUrl, RicardianContract, TokenData,
};
use rgbstd::txout::BlindSeal;
use rgbstd::{Amount, ChainNet, ContractId, GenesisSeal, GraphSeal, TokenIndex, Txid};
use schemata::{
    CollectibleFungibleAsset, InflatableFungibleAsset, NonInflatableAsset, PermissionedFungibleAsset,
    UniqueDigitalAsset,
};
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

type Res<T> = Result<T, String>;

fn str_field<'a>(p: &'a Value, key: &str) -> Res<&'a str> {
    p.get(key).and_then(Value::as_str).ok_or_else(|| format!("missing string field `{key}`"))
}

fn opt_str<'a>(p: &'a Value, key: &str) -> Option<&'a str> { p.get(key).and_then(Value::as_str) }

/// A number given as a JSON number or a decimal string (for values above 2^53).
fn u64_field(p: &Value, key: &str) -> Res<u64> {
    match p.get(key) {
        Some(Value::Number(n)) => n.as_u64().ok_or_else(|| format!("`{key}` must be a whole number")),
        Some(Value::String(s)) => s.parse().map_err(|_| format!("`{key}` must be a whole number")),
        _ => Err(format!("missing number field `{key}`")),
    }
}

/// Test networks only: rgbjs is early work and must not mint mainnet assets.
fn test_network(p: &Value) -> Res<(ChainNet, rgbstd::bitcoin::Network)> {
    use rgbstd::bitcoin::Network;
    Ok(match str_field(p, "network")? {
        "regtest" => (ChainNet::BitcoinRegtest, Network::Regtest),
        "testnet3" => (ChainNet::BitcoinTestnet3, Network::Testnet),
        "testnet4" => (ChainNet::BitcoinTestnet4, Network::Testnet4),
        "signet" => (ChainNet::BitcoinSignet, Network::Signet),
        "bitcoin" | "mainnet" => return Err("mainnet is not supported: rgbjs creates on test networks only".into()),
        other => return Err(format!("unknown network `{other}`; use regtest, testnet3, testnet4 or signet")),
    })
}

fn outpoint(p: &Value, key: &str) -> Res<(Txid, u32)> {
    let s = str_field(p, key)?;
    let (txid, vout) = s.split_once(':').ok_or_else(|| format!("`{key}` must be txid:vout"))?;
    let txid = Txid::from_str(txid).map_err(|e| format!("`{key}`: {e}"))?;
    let vout = vout.parse().map_err(|_| format!("`{key}`: invalid output number"))?;
    Ok((txid, vout))
}

fn precision(p: &Value) -> Res<Precision> {
    let n = u64_field(p, "precision")?;
    u8::try_from(n)
        .ok()
        .and_then(|n| Precision::try_from(n).ok())
        .ok_or_else(|| "`precision` must be 0 to 18 decimal places".into())
}

/// Issues a v0.11 contract and returns its `RGB\0CON` consignment file.
///
/// `params` is JSON: `schema` (NIA, CFA, UDA, IFA or PFA), `network`, `issuer`
/// (an identity string such as `ssi:anonymous`), `name`, `ticker` (not for
/// CFA), `precision`, `supply` (not for UDA), `terms`, `timestamp` (Unix
/// seconds), `outpoint` (`txid:vout` that will own the asset) and `blinding`
/// (a secret random u64). IFA also takes `maxSupply`, `inflationOutpoint`,
/// `inflationBlinding` and optionally `rejectListUrl`; PFA takes `pubkey`
/// (33-byte hex); UDA takes `tokenIndex`.
#[wasm_bindgen(js_name = issueContract)]
pub fn issue_contract(params: &str) -> Res<Vec<u8>> {
    let p: Value = serde_json::from_str(params).map_err(|e| format!("parameters: {e}"))?;
    let (chain, _) = test_network(&p)?;
    let (txid, vout) = outpoint(&p, "outpoint")?;
    let seal = GenesisSeal::from(BlindSeal::with_blinding(txid, vout, u64_field(&p, "blinding")?));
    let issuer = rgbstd::Identity::from_str(str_field(&p, "issuer")?).map_err(|e| format!("`issuer`: {e}"))?;
    let name = str_field(&p, "name")?;
    let terms = ContractTerms {
        text: RicardianContract::from_str(opt_str(&p, "terms").unwrap_or("")).map_err(|e| e.to_string())?,
        media: None,
    };
    let timestamp = p.get("timestamp").and_then(Value::as_i64).ok_or("missing number field `timestamp`")?;
    let spec = |ticker: &str| -> Res<AssetSpec> {
        AssetSpec::with(ticker, name, precision(&p)?, opt_str(&p, "details")).map_err(|e| e.to_string())
    };
    let bad = |e: rgbstd::contract::BuilderError| e.to_string();

    let mut stock = Stock::in_memory();
    let schema = str_field(&p, "schema")?;
    let builder = match schema {
        "NIA" | "CFA" | "IFA" | "PFA" | "UDA" => {
            macro_rules! kit {
                ($w:ty) => {{
                    stock.import_kit(<$w>::kit()).map_err(|e| e.to_string())?;
                    stock.contract_builder(issuer.clone(), <$w>::schema().schema_id(), chain).map_err(|e| e.to_string())?
                }};
            }
            match schema {
                "NIA" => {
                    let supply = u64_field(&p, "supply")?;
                    kit!(NonInflatableAsset)
                        .add_global_state("spec", spec(str_field(&p, "ticker")?)?)
                        .map_err(bad)?
                        .add_global_state("terms", terms)
                        .map_err(bad)?
                        .add_global_state("issuedSupply", Amount::from(supply))
                        .map_err(bad)?
                        .add_fungible_state("assetOwner", seal, supply)
                        .map_err(bad)?
                }
                "CFA" => {
                    let supply = u64_field(&p, "supply")?;
                    let mut b = kit!(CollectibleFungibleAsset)
                        .add_global_state("name", Name::try_from(name.to_owned()).map_err(|e| e.to_string())?)
                        .map_err(bad)?
                        .add_global_state("precision", precision(&p)?)
                        .map_err(bad)?;
                    if let Some(d) = opt_str(&p, "details") {
                        b = b
                            .add_global_state("details", rgbstd::stl::Details::from_str(d).map_err(|e| e.to_string())?)
                            .map_err(bad)?;
                    }
                    b.add_global_state("terms", terms)
                        .map_err(bad)?
                        .add_global_state("issuedSupply", Amount::from(supply))
                        .map_err(bad)?
                        .add_fungible_state("assetOwner", seal, supply)
                        .map_err(bad)?
                }
                "IFA" => {
                    let supply = u64_field(&p, "supply")?;
                    let max = u64_field(&p, "maxSupply")?;
                    if max < supply {
                        return Err("`maxSupply` is below `supply`".into());
                    }
                    let (itxid, ivout) = outpoint(&p, "inflationOutpoint")?;
                    let iseal = GenesisSeal::from(BlindSeal::with_blinding(itxid, ivout, u64_field(&p, "inflationBlinding")?));
                    let mut b = kit!(InflatableFungibleAsset)
                        .add_global_state("spec", spec(str_field(&p, "ticker")?)?)
                        .map_err(bad)?
                        .add_global_state("terms", terms)
                        .map_err(bad)?
                        .add_global_state("issuedSupply", Amount::from(supply))
                        .map_err(bad)?
                        .add_global_state("maxSupply", Amount::from(max))
                        .map_err(bad)?;
                    if let Some(url) = opt_str(&p, "rejectListUrl") {
                        b = b
                            .add_global_state("rejectListUrl", RejectListUrl::from_str(url).map_err(|e| e.to_string())?)
                            .map_err(bad)?;
                    }
                    b = b.add_fungible_state("assetOwner", seal, supply).map_err(bad)?;
                    if max > supply {
                        b = b.add_fungible_state("inflationAllowance", iseal, max - supply).map_err(bad)?;
                    }
                    b
                }
                "PFA" => {
                    let supply = u64_field(&p, "supply")?;
                    let key = str_field(&p, "pubkey")?;
                    let key: Vec<u8> = (0..key.len())
                        .step_by(2)
                        .map(|i| key.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok()))
                        .collect::<Option<_>>()
                        .ok_or("`pubkey` must be hex")?;
                    let pubkey = rgbstd::bitcoin::CompressedPublicKey::from_slice(&key)
                        .map_err(|_| "`pubkey` must be a 33-byte compressed public key")?;
                    kit!(PermissionedFungibleAsset)
                        .add_global_state("spec", spec(str_field(&p, "ticker")?)?)
                        .map_err(bad)?
                        .add_global_state("terms", terms)
                        .map_err(bad)?
                        .add_global_state("issuedSupply", Amount::from(supply))
                        .map_err(bad)?
                        .add_fungible_state("assetOwner", seal, supply)
                        .map_err(bad)?
                        .add_global_state("pubkey", pubkey)
                        .map_err(bad)?
                }
                _ => {
                    let index = TokenIndex::from_inner(u32::try_from(u64_field(&p, "tokenIndex")?).map_err(|_| "`tokenIndex` is too large")?);
                    let token = TokenData {
                        index,
                        preview: Some(EmbeddedMedia {
                            ty: MediaType::with("text/plain"),
                            data: SmallBlob::try_from_iter(name.bytes()).map_err(|e| e.to_string())?,
                        }),
                        ..Default::default()
                    };
                    kit!(UniqueDigitalAsset)
                        .add_global_state("spec", spec(str_field(&p, "ticker")?)?)
                        .map_err(bad)?
                        .add_global_state("terms", terms)
                        .map_err(bad)?
                        .add_global_state("tokens", token)
                        .map_err(bad)?
                        .add_data("assetOwner", seal, Allocation::with(index, 1))
                        .map_err(bad)?
                }
            }
        }
        other => return Err(format!("unknown schema `{other}`; use NIA, CFA, UDA, IFA or PFA")),
    };
    let contract = builder.issue_contract_raw(timestamp).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    contract.save(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Creates a v0.11 invoice paying to a blinded seal on one of your outputs.
///
/// `params` is JSON: `network`, `outpoint` (`txid:vout` you control),
/// `blinding` (a secret random u64), and optionally `contractId`, `amount`
/// (for fungible assets) or `tokenIndex` (for UDA), `assignmentName`,
/// `expiry` (Unix seconds) and `endpoints` (transport URLs). Returns JSON with
/// the `invoice` and the `seal` you must keep to claim what arrives.
#[wasm_bindgen(js_name = createInvoice)]
pub fn create_invoice(params: &str) -> Res<String> {
    let p: Value = serde_json::from_str(params).map_err(|e| format!("parameters: {e}"))?;
    let (_, network) = test_network(&p)?;
    let (txid, vout) = outpoint(&p, "outpoint")?;
    let blinding = u64_field(&p, "blinding")?;
    let seal = GraphSeal::with_blinding(txid, vout, blinding);
    let secret = seal.to_secret_seal();
    let mut b = RgbInvoiceBuilder::new(XChainNet::bitcoin(network, Beneficiary::BlindedSeal(secret)));
    if let Some(id) = opt_str(&p, "contractId") {
        b = b.set_contract(ContractId::from_str(id).map_err(|e| format!("`contractId`: {e}"))?);
    }
    if p.get("amount").is_some() {
        b = b.set_amount_raw(u64_field(&p, "amount")?);
    } else if p.get("tokenIndex").is_some() {
        let index = u32::try_from(u64_field(&p, "tokenIndex")?).map_err(|_| "`tokenIndex` is too large")?;
        b = b.set_allocation_raw(Allocation::with(TokenIndex::from_inner(index), 1));
    }
    if let Some(name) = opt_str(&p, "assignmentName") {
        b = b.set_assignment_name(strict_types::FieldName::try_from(name.to_owned()).map_err(|e| e.to_string())?);
    }
    if let Some(expiry) = p.get("expiry").and_then(Value::as_i64) {
        b = b.set_expiry_timestamp(expiry);
    }
    for e in p.get("endpoints").and_then(Value::as_array).into_iter().flatten() {
        let e = e.as_str().ok_or("`endpoints` must be strings")?;
        b = b.add_transport(e).map_err(|(_, err)| format!("endpoint `{e}`: {err}"))?;
    }
    let invoice = b.finish();
    Ok(json!({
        "line": "v0.11",
        "invoice": invoice.to_string(),
        "secretSeal": secret.to_string(),
        "seal": { "outpoint": format!("{txid}:{vout}"), "blinding": blinding.to_string() },
    })
    .to_string())
}


/// Checks whether a transfer pays you: rebuilds the blinded seal of your
/// invoice from `outpoint` and the secret `blinding`, checks that the transfer
/// is addressed to it, and lists what arrives on your outpoint. Returns the
/// validation result with `receive: { secretSeal, addressedToYou, received }`.
#[wasm_bindgen(js_name = receive)]
pub fn receive(bytes: &[u8], outpoint: &str, blinding: &str) -> Res<String> {
    let p = json!({ "outpoint": outpoint, "blinding": blinding });
    let (txid, vout) = self::outpoint(&p, "outpoint")?;
    let blinding = u64_field(&p, "blinding")?;
    let seal = GraphSeal::with_blinding(txid, vout, blinding);
    let secret = seal.to_secret_seal().to_string();
    let mut out: Value = serde_json::from_str(&crate::validate_with(bytes, Some(seal))?).map_err(|e| e.to_string())?;
    let named = out["terminals"]
        .as_array()
        .map(|ts| ts.iter().any(|t| t["seals"].as_array().is_some_and(|s| s.iter().any(|x| x.as_str() == Some(secret.as_str())))))
        .unwrap_or(false);
    let here = format!("{txid}:{vout}");
    let mut received = vec![];
    if let Some(owned) = out["state"]["owned"].as_object() {
        for (name, cells) in owned {
            for c in cells.as_array().into_iter().flatten() {
                if c["seal"].as_str() == Some(here.as_str()) && !c["witness"].is_null() && c["spent"] == json!(false) {
                    received.push(json!({ "name": name, "value": c["value"], "outpoint": here, "witness": c["witness"] }));
                }
            }
        }
    }
    out["receive"] = json!({ "secretSeal": secret, "addressedToYou": named, "received": received });
    Ok(out.to_string())
}
