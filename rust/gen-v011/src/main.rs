//! Makes a v0.11 transfer consignment without any indexer or network.
//!
//!   gen-v011 <out dir> [nia|cfa|uda|ifa|pfa]...
//!
//! A wpkh wallet from a fixed test seed gets a made-up funding transaction
//! (never broadcast) through an in-process indexer. A NIA contract is issued
//! on its output, and the wallet pays a blinded invoice for 100 units with
//! `RgbWallet::pay`, the call `rgb transfer` makes. The witness transaction
//! stays unsigned: client-side validation doesn't need signatures.

#[macro_use]
extern crate amplify;

use std::convert::Infallible;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::str::FromStr;

use amplify::confinement::Confined;
use bpstd::{
    h, BlockHash, HardenedIndex, LockTime, Network, Outpoint, Sats, SeqNo, Tx, TxIn,
    TxOut, TxVer, Txid, Witness, XpubDerivable, Xpriv,
};
use bpwallet::{
    Indexer, Layer2, MayError, MiningInfo, Party, TxCredit, TxDebit, TxStatus, Wallet, WalletAddr,
    WalletCache, WalletDescr, WalletTx,
};
use rgb::pay::TransferParams;
use rgb::{RgbDescr, RgbWallet, WpkhDescr};
use rgbstd::containers::{ConsignmentExt, FileContent};
use rgbstd::contract::IssuerWrapper;
use rgbstd::invoice::{Beneficiary, Precision, RgbInvoiceBuilder, XChainNet};
use rgbstd::persistence::Stock;
use rgbstd::stl::{
    AssetSpec, ContractTerms, EmbeddedMedia, MediaType, Name, RejectListUrl, RicardianContract, TokenData,
};
use rgbstd::{Allocation, TokenIndex};
use amplify::confinement::SmallBlob;
use amplify::Wrapper;
use rgbstd::txout::BlindSeal;
use rgbstd::{Amount, ChainNet, GenesisSeal, GraphSeal};
use schemata::dumb::NoResolver;
use schemata::{
    CollectibleFungibleAsset, InflatableFungibleAsset, NonInflatableAsset, PermissionedFungibleAsset, UniqueDigitalAsset,
};

/// An indexer that knows exactly one transaction and can't publish.
struct Offline(Tx);

impl Indexer for Offline {
    type Error = Infallible;

    fn create<K, D: bpwallet::Descriptor<K>, L2: Layer2>(
        &self,
        _: &WalletDescr<K, D, L2::Descr>,
    ) -> MayError<WalletCache<L2::Cache>, Vec<Infallible>> {
        unimplemented!("only update is used")
    }

    fn update<K, D: bpwallet::Descriptor<K>, L2: Layer2>(
        &self,
        descr: &WalletDescr<K, D, L2::Descr>,
        cache: &mut WalletCache<L2::Cache>,
    ) -> MayError<usize, Vec<Infallible>> {
        let tx = &self.0;
        let txid = tx.txid();
        let mut wtx = WalletTx {
            txid,
            status: TxStatus::Mined(MiningInfo {
                height: NonZeroU32::new(110).unwrap(),
                time: 1_700_000_000,
                block_hash: BlockHash::from([0x22; 32]),
            }),
            inputs: tx
                .inputs
                .iter()
                .map(|i| TxCredit {
                    outpoint: i.prev_output,
                    sequence: i.sequence,
                    coinbase: false,
                    script_sig: i.sig_script.clone(),
                    witness: i.witness.clone(),
                    value: Sats::ZERO.into(),
                    payer: Party::Subsidy,
                })
                .collect(),
            outputs: tx
                .outputs
                .iter()
                .enumerate()
                .map(|(n, o)| TxDebit {
                    outpoint: Outpoint::new(txid, n as u32),
                    beneficiary: Party::from(o.script_pubkey.clone()),
                    value: o.value.into(),
                    spent: None,
                })
                .collect(),
            fee: Sats::ZERO,
            size: 0,
            weight: 0,
            version: tx.version,
            locktime: tx.lock_time,
        };
        for keychain in descr.keychains() {
            for derive in descr.addresses(keychain).take(20) {
                let script = derive.addr.script_pubkey();
                let mut addr = WalletAddr::<i64>::from(derive);
                for debit in &mut wtx.outputs {
                    if debit.beneficiary.script_pubkey().as_ref() == Some(&script) {
                        cache.utxo.insert(debit.outpoint);
                        debit.beneficiary = Party::from_wallet_addr(&addr);
                        addr.used = addr.used.saturating_add(1);
                        addr.volume.saturating_add_assign(debit.value);
                        addr.balance = addr.balance.saturating_add(debit.value.sats() as i64);
                    }
                }
                cache.addr.entry(addr.terminal.keychain).or_default().insert(addr.expect_transmute());
            }
        }
        cache.tx.insert(txid, wtx);
        MayError::ok(1)
    }

    fn publish(&self, _: &Tx) -> Result<(), Infallible> { panic!("gen-v011 never broadcasts") }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("usage: gen-v011 <out dir> [nia|cfa|uda|ifa|pfa]..."));
    let schemas: Vec<String> = args.collect();
    let schemas = if schemas.is_empty() { vec![s!("nia")] } else { schemas };
    for (k, schema) in schemas.iter().enumerate() {
        build(schema, k as u8, &out);
    }
}

/// Issues a contract of `schema` on a made-up funding transaction and pays a
/// blinded invoice from it. `k` varies the funding transaction per schema.
fn build(schema: &str, k: u8, out: &std::path::Path) {
    // A test key from a fixed seed: public, never holds anything real.
    let master = Xpriv::new_master(true, b"rgbjs test vectors: not a real wallet");
    let path = [h!(84), h!(1), h!(0)];
    let account = master.derive_priv(&path).to_xpub();
    let descr = format!("[{}/84h/1h/0h]{account}/<0;1;9>/*", master.fingerprint());
    let xpub = XpubDerivable::from_str(&descr).expect("descriptor");
    let descr = RgbDescr::Wpkh(WpkhDescr::with_key(xpub));

    // Two outputs to the wallet: IFA puts its inflation allowance on the second.
    let spks: Vec<_> = {
        let w = Wallet::<XpubDerivable, RgbDescr<XpubDerivable>>::new_layer1(descr.clone(), Network::Regtest);
        w.addresses(0u8).take(2).map(|a| a.addr.script_pubkey()).collect()
    };
    let funding = Tx {
        version: TxVer::V2,
        inputs: Confined::from_checked(vec![TxIn {
            prev_output: Outpoint::new(Txid::from([0x11 + k; 32]), 0),
            sig_script: none!(),
            sequence: SeqNo::from_consensus_u32(0xFFFF_FFFD),
            witness: Witness::default(),
        }]),
        outputs: Confined::from_checked(
            spks.into_iter().map(|spk| TxOut::new(spk, Sats::from_sats(50_000u64))).collect(),
        ),
        lock_time: LockTime::ZERO,
    };
    let funding_txid = funding.txid();
    let mut wallet = Wallet::new_layer1(descr, Network::Regtest);
    wallet.update(&Offline(funding)).into_result().expect("cache");

    let mut stock = Stock::in_memory();
    let txid = rgbstd::Txid::from_str(&funding_txid.to_string()).unwrap();
    let seal = |vout: u32| GenesisSeal::from(BlindSeal::with_blinding(txid, vout, 7 + vout as u64));
    let terms = ContractTerms { text: RicardianContract::from_str("Test vector for rgbjs. Not a real asset.").unwrap(), media: None };
    let chain = ChainNet::BitcoinRegtest;
    let token = TokenIndex::from_inner(2);
    let (contract, name) = match schema {
        "nia" => {
            stock.import_kit(NonInflatableAsset::kit()).unwrap();
            let c = stock
                .contract_builder("ssi:rgbjs-test", NonInflatableAsset::schema().schema_id(), chain)
                .unwrap()
                .add_global_state("spec", AssetSpec::new("DEMO", "Demo Asset", Precision::Indivisible))
                .unwrap()
                .add_global_state("terms", terms)
                .unwrap()
                .add_global_state("issuedSupply", Amount::from(1_000_000u64))
                .unwrap()
                .add_fungible_state("assetOwner", seal(0), 1_000_000u64)
                .unwrap();
            (c, "transfer-demo")
        }
        "cfa" => {
            stock.import_kit(CollectibleFungibleAsset::kit()).unwrap();
            let c = stock
                .contract_builder("ssi:rgbjs-test", CollectibleFungibleAsset::schema().schema_id(), chain)
                .unwrap()
                .add_global_state("name", Name::from("Demo Collectible"))
                .unwrap()
                .add_global_state("precision", Precision::CentiMicro)
                .unwrap()
                .add_global_state("terms", terms)
                .unwrap()
                .add_global_state("issuedSupply", Amount::from(100_000u64))
                .unwrap()
                .add_fungible_state("assetOwner", seal(0), 100_000u64)
                .unwrap();
            (c, "cfa")
        }
        "uda" => {
            stock.import_kit(UniqueDigitalAsset::kit()).unwrap();
            let preview = EmbeddedMedia {
                ty: MediaType::with("image/*"),
                data: SmallBlob::try_from_iter(vec![0, 0]).unwrap(),
            };
            let token_data = TokenData { index: token, preview: Some(preview), ..Default::default() };
            let c = stock
                .contract_builder("ssi:rgbjs-test", UniqueDigitalAsset::schema().schema_id(), chain)
                .unwrap()
                .add_global_state("spec", AssetSpec::new("UDEMO", "Demo Unique", Precision::Indivisible))
                .unwrap()
                .add_global_state("terms", terms)
                .unwrap()
                .add_global_state("tokens", token_data)
                .unwrap()
                .add_data("assetOwner", seal(0), Allocation::with(token, 1))
                .unwrap();
            (c, "uda")
        }
        "ifa" => {
            stock.import_kit(InflatableFungibleAsset::kit()).unwrap();
            let c = stock
                .contract_builder("ssi:rgbjs-test", InflatableFungibleAsset::schema().schema_id(), chain)
                .unwrap()
                .add_global_state("spec", AssetSpec::new("IDEMO", "Demo Inflatable", Precision::CentiMicro))
                .unwrap()
                .add_global_state("terms", terms)
                .unwrap()
                .add_global_state("issuedSupply", Amount::from(100_000u64))
                .unwrap()
                .add_global_state("maxSupply", Amount::from(150_000u64))
                .unwrap()
                .add_global_state("rejectListUrl", RejectListUrl::from("example.com/reject"))
                .unwrap()
                .add_fungible_state("assetOwner", seal(0), 100_000u64)
                .unwrap()
                .add_fungible_state("inflationAllowance", seal(1), 50_000u64)
                .unwrap();
            (c, "ifa")
        }
        "pfa" => {
            stock.import_kit(PermissionedFungibleAsset::kit()).unwrap();
            // A fixed test key (sha256 of a label): PFA transfers must be signed by it.
            let pubkey = rgbstd::bitcoin::CompressedPublicKey::from_slice(&pfa_key().public_key().serialize()).unwrap();
            let c = stock
                .contract_builder("ssi:rgbjs-test", PermissionedFungibleAsset::schema().schema_id(), chain)
                .unwrap()
                .add_global_state("spec", AssetSpec::new("PDEMO", "Demo Permissioned", Precision::CentiMicro))
                .unwrap()
                .add_global_state("terms", terms)
                .unwrap()
                .add_global_state("issuedSupply", Amount::from(100_000u64))
                .unwrap()
                .add_fungible_state("assetOwner", seal(0), 100_000u64)
                .unwrap()
                .add_global_state("pubkey", pubkey)
                .unwrap();
            (c, "pfa")
        }
        other => panic!("unknown schema {other}"),
    };
    let contract = contract.issue_contract_raw(1_761_350_000).expect("issue");
    let contract_id = contract.contract_id();
    contract.save_file(out.join(format!("{name}.contract.rgb"))).unwrap();
    stock.import_contract(contract, NoResolver).unwrap();
    println!("{schema}: funding {funding_txid}, contract {contract_id}");

    // Bob's blinded seal on an outpoint of his own; only its hash goes in the invoice.
    let bob = GraphSeal::new_random(rgbstd::Txid::from_str(&"33".repeat(32)).unwrap(), k as u32);
    let builder = RgbInvoiceBuilder::new(XChainNet::bitcoin(
        rgbstd::bitcoin::Network::Regtest,
        Beneficiary::BlindedSeal(bob.to_secret_seal()),
    ))
    .set_contract(contract_id)
    .set_assignment_name(strict_types::FieldName::from("assetOwner"));
    let invoice = if schema == "uda" {
        builder.set_allocation_raw(Allocation::with(token, 1))
    } else {
        builder.set_amount_raw(100u64)
    }
    .finish();
    println!("{schema}: invoice {invoice}");

    let mut wallet = RgbWallet::new(stock, wallet);
    let (_psbt, _, mut transfer) = wallet
        .pay::<bpstd::psbt::PropKey, bpstd::psbt::Output>(&invoice, TransferParams::with(1000, 1000))
        .expect("pay");
    if schema == "pfa" {
        // rgb-api doesn't sign PFA transitions. The signature isn't part of the
        // operation ID (Transition::commit), so sign each one here: an ECDSA
        // signature over the transition ID, as the PFA script's `vts` checks.
        // Save the unsigned transfer too, as a vector the validator must reject.
        transfer.save_file(out.join(format!("{name}-unsigned.transfer.rgb"))).unwrap();
        let mut bundles = transfer.bundles.clone().release();
        for wb in &mut bundles {
            let mut known = wb.bundle.known_transitions.clone().release();
            for kt in &mut known {
                let msg = rgbstd::secp256k1::Message::from_digest(kt.opid.to_byte_array());
                let sig = rgbstd::secp256k1::SECP256K1.sign_ecdsa(msg, &pfa_key()).serialize_compact();
                kt.transition.signature = Some(rgbstd::Signature::from(amplify::Bytes64::from(sig)));
            }
            wb.bundle.known_transitions = Confined::from_checked(known);
        }
        transfer.bundles = Confined::from_checked(bundles);
    }
    transfer.save_file(out.join(format!("{name}.transfer.rgb"))).unwrap();
}

/// The PFA issuer key for the test vectors: the SHA-256 of a fixed label.
/// Public by design; it never controls anything real.
fn pfa_key() -> rgbstd::secp256k1::SecretKey {
    use rgbstd::bitcoin::hashes::{sha256, Hash};
    let digest = sha256::Hash::hash(b"rgbjs PFA test issuer key: not a real key");
    rgbstd::secp256k1::SecretKey::from_byte_array(digest.to_byte_array()).expect("valid key")
}
