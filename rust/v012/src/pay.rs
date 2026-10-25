//! Paying a v0.12 invoice offline: builds the witness transaction as an
//! unsigned PSBT and the transfer consignment for the payee.
//!
//! The wallet here is rgb-runtime's own `Owner` over an in-memory `Holder`:
//! the caller gives an xpub descriptor, the coins it may spend (with their
//! previous transactions) and the seals of invoices it was paid to. Nothing
//! is signed or broadcast; the caller signs the PSBT with its own keys.

use std::collections::{BTreeSet, HashMap};
use std::convert::Infallible;
use std::str::FromStr;

use amplify::MultiError;
use bp::seals::{TxoSeal, WTxoSeal};
use bpstd::psbt::{TxParams, Utxo};
use bpstd::{Network, Outpoint, Sats, ScriptPubkey, Terminal, Tx, Txid, UnsignedTx, XpubDerivable};
use commit_verify::{Digest, DigestExt, Sha256};
use hypersonic::{Articles, CodexId, ContractId, Issuer};
use rgb::invoice::RgbInvoice;
use rgb::popls::bp::RgbWallet;
use rgb::{
    Consensus, Consignment, ConsumeError, Contract, Contracts, CreateParams, IssuerError, Pile, RgbSeal,
    Stock, Stockpile, WitnessStatus,
};
use rgbp::descriptors::RgbDescr;
use rgbp::resolvers::{Resolver, ResolverError};
use amplify::ByteArray;
use bpstd::{TapScript, TapTree};
use commit_verify::CommitVerify;
use rgbp::{CoinselectStrategy, Holder, MemUtxos, Owner, Payment, RgbRuntime, UtxoSet};
use serde::Deserialize;
use serde_json::json;
use strict_encoding::{StreamReader, StreamWriter, StrictReader, StrictWriter};
use wasm_bindgen::prelude::*;

use crate::mem::{NotPersistent, PileMem, StockMem};

type Res<T> = Result<T, String>;
type Runtime = RgbRuntime<Owner<Offline, Holder>, MemStockpile>;

/// Knows only the transactions it was given, and never broadcasts.
struct Offline(HashMap<Txid, UnsignedTx>);

impl Resolver for Offline {
    fn resolve_tx(&self, txid: Txid) -> Result<Option<UnsignedTx>, ResolverError> { Ok(self.0.get(&txid).cloned()) }
    fn resolve_tx_status(&self, _: Txid) -> Result<WitnessStatus, ResolverError> { Ok(WitnessStatus::Tentative) }
    fn resolve_utxos(
        &self,
        _: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>> {
        std::iter::empty()
    }
    fn last_block_height(&self) -> Result<u64, ResolverError> { Ok(0) }
    fn broadcast(&self, _: &Tx) -> Result<(), ResolverError> { Err(ResolverError::Logic) }
}

/// Holds contracts only for the length of one call; `Contracts` keeps them
/// in its own cache, so this only tracks which ones exist.
#[derive(Default)]
struct MemStockpile {
    ids: BTreeSet<ContractId>,
}

impl Stockpile for MemStockpile {
    type Stock = StockMem;
    type Pile = PileMem;
    type Error = NotPersistent;

    fn consensus(&self) -> Consensus { Consensus::Bitcoin }
    fn is_testnet(&self) -> bool { true }
    fn issuers_count(&self) -> usize { 0 }
    fn contracts_count(&self) -> usize { self.ids.len() }
    fn has_issuer(&self, _: CodexId) -> bool { false }
    fn has_contract(&self, id: ContractId) -> bool { self.ids.contains(&id) }
    fn codex_ids(&self) -> impl Iterator<Item = CodexId> { std::iter::empty() }
    fn contract_ids(&self) -> impl Iterator<Item = ContractId> { self.ids.iter().copied() }
    fn issuer(&self, _: CodexId) -> Option<Issuer> { None }
    fn contract(&self, _: ContractId) -> Option<Contract<StockMem, PileMem>> { None }
    fn import_issuer(&mut self, _: Issuer) -> Result<Issuer, NotPersistent> { Err(NotPersistent) }

    fn import_contract(
        &mut self,
        articles: Articles,
        consignment: Consignment<TxoSeal>,
    ) -> Result<
        Contract<StockMem, PileMem>,
        MultiError<ConsumeError<<TxoSeal as RgbSeal>::Definition>, <StockMem as Stock>::Error, <PileMem as Pile>::Error>,
    > {
        let contract = Contract::with(articles, consignment, ())?;
        self.ids.insert(contract.contract_id());
        Ok(contract)
    }

    fn issue(
        &mut self,
        params: CreateParams<<TxoSeal as RgbSeal>::Definition>,
    ) -> Result<Contract<StockMem, PileMem>, MultiError<IssuerError, NotPersistent, NotPersistent>> {
        Err(MultiError::A(IssuerError::UnknownCodex(params.issuer.codex_id())))
    }

    fn purge(&mut self, contract_id: ContractId) -> Result<(), NotPersistent> {
        self.ids.remove(&contract_id);
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Coin {
    /// `txid:vout`
    outpoint: String,
    sats: u64,
    /// Where the descriptor derives its key, such as `&0/3`.
    terminal: String,
    /// The transaction that created it, as hex.
    tx: String,
}

#[derive(Deserialize)]
struct SealRef {
    outpoint: String,
    nonce: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PayParams {
    /// An xpub descriptor key for BIP86 taproot, such as
    /// `[fp/86h/1h/0h]tpub.../<0;1>/*`.
    descriptor: String,
    coins: Vec<Coin>,
    /// Seals of invoices this wallet was paid to: the outpoint and nonce
    /// from `createInvoice`.
    #[serde(default)]
    seals: Vec<SealRef>,
    invoice: String,
    fee: u64,
    /// Sats to send with the payment when the invoice asks for a new output
    /// (a witness-out invoice) rather than naming an existing one.
    #[serde(default)]
    giveaway: Option<u64>,
    /// How the witness transaction commits: `opret` (an OP_RETURN output,
    /// the default) or `tapret` (inside the change output's taproot key, so
    /// the transaction has no OP_RETURN and looks like any taproot payment).
    #[serde(default)]
    commit: Option<String>,
}

/// Pays a v0.12 invoice from the given wallet.
///
/// `contracts` is every consignment file that makes up the wallet's history
/// of the contract (the issuer's backup and any transfers received),
/// concatenated with a 4-byte big-endian length before each. `params` is
/// JSON (see `PayParams`). `noise_seed` is the 32-byte secret the wallet's
/// seals and invoices were made with.
///
/// Returns JSON with the unsigned `psbt` (base64, version 0), the witness
/// `txid` and unsigned `tx` (hex), the `change` output if any, the `transfer`
/// consignment for the payee, and the payer's updated `backup` (both hex).
#[wasm_bindgen(js_name = payInvoice)]
pub fn pay_invoice(contracts: &[u8], params: &str, noise_seed: &[u8]) -> Res<String> {
    let params: PayParams = serde_json::from_str(params).map_err(|e| format!("parameters: {e}"))?;
    let seed: [u8; 32] = noise_seed.try_into().map_err(|_| "the noise seed must be exactly 32 bytes".to_string())?;
    let invoice = RgbInvoice::<ContractId>::from_str(&params.invoice).map_err(|e| format!("invoice: {e}"))?;
    // Test networks only, like issuing: a mainnet invoice is refused before
    // anything is built.
    let parsed: serde_json::Value = serde_json::from_str(&crate::parse_invoice(&params.invoice)?).map_err(|e| e.to_string())?;
    if parsed["testnet"] != serde_json::Value::Bool(true) {
        return Err("mainnet is not supported: rgbjs pays on test networks only".into());
    }
    let xpub = XpubDerivable::from_str(&params.descriptor).map_err(|e| format!("descriptor: {e}"))?;

    let mut descriptor = RgbDescr::key_only_unfunded(xpub, seed);
    let mut engine = Sha256::new();
    engine.input_raw(&seed);
    for s in &params.seals {
        let outpoint = Outpoint::from_str(&s.outpoint).map_err(|e| format!("seal outpoint: {e}"))?;
        descriptor.add_seal(WTxoSeal::no_fallback(outpoint, engine.clone(), s.nonce));
    }

    let mut utxos = MemUtxos::default();
    let mut txs = HashMap::new();
    for c in &params.coins {
        let outpoint = Outpoint::from_str(&c.outpoint).map_err(|e| format!("coin outpoint: {e}"))?;
        let terminal = Terminal::from_str(&c.terminal).map_err(|e| format!("coin terminal: {e}"))?;
        let tx = Tx::from_str(&c.tx).map_err(|e| format!("coin transaction: {e}"))?;
        if tx.txid() != outpoint.txid {
            return Err(format!("the transaction given for {outpoint} has txid {}", tx.txid()));
        }
        let out = tx.outputs.get(outpoint.vout.into_usize()).ok_or(format!("{outpoint} has no such output"))?;
        if out.value != Sats::from_sats(c.sats) {
            return Err(format!("{outpoint} holds {} sats, not {}", out.value, c.sats));
        }
        utxos.insert(outpoint, out.value, terminal);
        txs.insert(outpoint.txid, UnsignedTx::from(tx));
    }

    let wallet = Owner::with_components(Network::Testnet4, Holder::with_components(descriptor, utxos), Offline(txs));
    let contracts_db: Contracts<MemStockpile> = Contracts::load(MemStockpile::default());
    let mut runtime: Runtime = RgbRuntime::from(RgbWallet::with_components(wallet, contracts_db));

    // Each file is read twice when needed: the first import of an unknown
    // contract takes only the seals inside the file, then consuming it again
    // resolves this wallet's own seals.
    let files = split(contracts)?;
    for allow_unknown in [true, false] {
        for file in &files {
            if allow_unknown && runtime.contracts.has_contract(invoice.scope) {
                break;
            }
            if file.len() < 10 || &file[..8] != crate::CONSIGN_MAGIC {
                return Err("not an RGB v0.12 consignment file (missing RGBCNSGN header)".into());
            }
            let mut reader = StrictReader::with(StreamReader::new::<{ usize::MAX }>(&file[10..]));
            runtime
                .consume(allow_unknown, &mut reader, |_, _, _| -> Result<_, Infallible> { Ok(()) })
                .map_err(|e| format!("consignment: {e}"))?;
        }
    }
    if !runtime.contracts.has_contract(invoice.scope) {
        return Err(format!("none of the files is for contract {}", invoice.scope));
    }

    let tx_params = TxParams::with(Sats::from_sats(params.fee));
    let giveaway = params.giveaway.map(Sats::from_sats);
    let (psbt, payment) = match params.commit.as_deref().unwrap_or("opret") {
        "opret" => runtime
            .pay_invoice(&invoice, CoinselectStrategy::Aggregate, tx_params, giveaway)
            .map_err(|e| format!("payment: {e}"))?,
        "tapret" => pay_tapret(&mut runtime, &invoice, tx_params, giveaway)?,
        other => return Err(format!("commit must be `opret` or `tapret`, not `{other}`")),
    };
    let tapret = psbt.outputs().find(|o| o.is_tapret_host() && o.tapret_commitment().is_ok()).map(|o| {
        let commitment = o.tapret_commitment().expect("checked");
        let root = TapTree::with_single_leaf(TapScript::commit(&commitment)).merkle_root();
        json!({ "vout": o.index(), "merkleRoot": hex(&root.to_byte_array()), "scriptPubkey": hex(o.script.as_slice()) })
    });

    let mut transfer = Vec::new();
    transfer.extend_from_slice(crate::CONSIGN_MAGIC);
    transfer.extend_from_slice(&rgb::CONSIGN_VERSION.to_be_bytes());
    runtime
        .contracts
        .consign(invoice.scope, &payment.terminals, StrictWriter::with(StreamWriter::new::<{ usize::MAX }>(&mut transfer)))
        .map_err(|e| format!("transfer: {e}"))?;

    // The payer's own copy, with the change it keeps, to pay from later.
    let mut backup = Vec::new();
    backup.extend_from_slice(crate::CONSIGN_MAGIC);
    backup.extend_from_slice(&rgb::CONSIGN_VERSION.to_be_bytes());
    runtime
        .contracts
        .export(invoice.scope, StrictWriter::with(StreamWriter::new::<{ usize::MAX }>(&mut backup)))
        .map_err(|e| format!("backup: {e}"))?;
    let tx: Tx = psbt.to_unsigned_tx().into();

    let change = payment.psbt_meta.change.map(|c| json!({ "vout": c.vout.to_u32(), "terminal": c.terminal.to_string() }));
    Ok(json!({
        "line": "v0.12",
        "psbt": psbt.to_base64_ver(bpstd::psbt::PsbtVer::V0),
        "txid": psbt.txid().to_string(),
        "fee": params.fee,
        "change": change,
        "tapret": tapret,
        "tx": tx.to_string(),
        "transfer": hex(&transfer),
        "backup": hex(&backup),
    })
    .to_string())
}

fn hex(b: &[u8]) -> String { b.iter().map(|b| format!("{b:02x}")).collect() }

/// `RgbRuntime::pay_invoice` with the change output as the tapret host
/// instead of an added OP_RETURN output: rgb-runtime 0.12.0-rc.3 always adds
/// one, so this runs its steps (fulfill, compose, resolve, bundle, fill,
/// complete) with that one difference.
fn pay_tapret(
    runtime: &mut Runtime,
    invoice: &RgbInvoice<ContractId>,
    params: TxParams,
    giveaway: Option<Sats>,
) -> Res<(bpstd::Psbt, Payment)> {
    use rgb::popls::bp::OpRequestSet;
    use rgb::invoice::RgbBeneficiary;
    use rgb::RgbSealDef;
    use rgbpsbt::{RgbPsbt, ScriptResolver};

    let request = runtime
        .fulfill(invoice, CoinselectStrategy::Aggregate, giveaway)
        .map_err(|e| format!("payment: {e}"))?;
    let script = OpRequestSet::with(request.clone());
    let (mut psbt, meta) = runtime.compose_psbt(&script, params).map_err(|e| format!("payment: {e}"))?;
    let change = meta.change.ok_or("a tapret commitment goes in the change output, and this payment has none")?;
    // The commitment goes in the first taproot output, which must be ours.
    let first = psbt.outputs().find(|o| o.script.is_p2tr()).map(|o| o.index());
    if first != Some(change.vout.into_usize()) {
        return Err("the payee's taproot output comes before the change, so the commitment can't go in the change; use commit: 'opret'".into());
    }
    psbt.output_mut(change.vout.into_usize())
        .expect("change output")
        .set_tapret_host()
        .map_err(|e| format!("tapret host: {e}"))?;
    psbt.complete_construction();
    let resolved = script
        .resolve_seals(psbt.script_resolver(), Some(change.vout))
        .map_err(|_| "the payment needs a change output for the remaining state".to_string())?;
    let bundle = runtime.bundle(resolved, Some(change.vout)).map_err(|e| format!("payment: {e}"))?;
    psbt.rgb_fill_csv(&bundle).map_err(|e| format!("payment: {e}"))?;
    let mut payment = Payment { uncomit_psbt: psbt.clone(), psbt_meta: meta, bundle, terminals: none!() };
    let psbt = runtime
        .complete(payment.uncomit_psbt.clone(), &payment.bundle)
        .map_err(|e| format!("payment: {e}"))?;
    let terminal = match invoice.auth {
        RgbBeneficiary::Token(auth) => auth,
        RgbBeneficiary::WitnessOut(wout) => request
            .resolve_seal(wout, psbt.script_resolver())
            .ok_or("the payee's output is missing from the transaction")?
            .auth_token(),
    };
    payment.terminals.insert(terminal);
    Ok((psbt, payment))
}

fn split(mut bytes: &[u8]) -> Res<Vec<&[u8]>> {
    let mut files = vec![];
    while !bytes.is_empty() {
        if bytes.len() < 4 {
            return Err("contracts: truncated length prefix".into());
        }
        let len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let rest = &bytes[4..];
        if rest.len() < len {
            return Err("contracts: a file is shorter than its length prefix".into());
        }
        files.push(&rest[..len]);
        bytes = &rest[len..];
    }
    if files.is_empty() {
        return Err("give at least one consignment file".into());
    }
    Ok(files)
}

/// The address and scriptPubKey a descriptor gives at a terminal such as
/// `&0/3`: where to receive coins this wallet can spend.
#[wasm_bindgen(js_name = deriveAddress)]
pub fn derive_address(descriptor: &str, terminal: &str) -> Res<String> {
    use bpstd::{Address, Derive};
    let xpub = XpubDerivable::from_str(descriptor).map_err(|e| format!("descriptor: {e}"))?;
    let terminal = Terminal::from_str(terminal).map_err(|e| format!("terminal: {e}"))?;
    let descr = RgbDescr::<XpubDerivable>::key_only_unfunded(xpub, [0u8; 32]);
    let spk = descr
        .derive(terminal.keychain, terminal.index)
        .next()
        .ok_or("the descriptor gives no script at that terminal")?
        .to_script_pubkey();
    let address = Address::with(&spk, Network::Testnet4).map_err(|e| e.to_string())?;
    Ok(json!({ "address": address.to_string(), "scriptPubkey": spk.as_slice().iter().map(|b| format!("{b:02x}")).collect::<String>() }).to_string())
}

