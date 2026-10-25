//! Makes a v0.11 transfer consignment without any indexer or network.
//!
//!   gen-v011 <out dir>
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
use rgbstd::stl::{AssetSpec, ContractTerms, RicardianContract};
use rgbstd::txout::BlindSeal;
use rgbstd::{Amount, ChainNet, GenesisSeal, GraphSeal};
use schemata::dumb::NoResolver;
use schemata::NonInflatableAsset;

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
    let out = PathBuf::from(std::env::args().nth(1).expect("out dir"));

    // A test key from a fixed seed: public, never holds anything real.
    let master = Xpriv::new_master(true, b"rgbjs test vectors: not a real wallet");
    let path = [h!(84), h!(1), h!(0)];
    let account = master.derive_priv(&path).to_xpub();
    let descr = format!("[{}/84h/1h/0h]{account}/<0;1;9>/*", master.fingerprint());
    let xpub = XpubDerivable::from_str(&descr).expect("descriptor");
    let descr = RgbDescr::Wpkh(WpkhDescr::with_key(xpub));

    let spk = {
        let mut w = Wallet::<XpubDerivable, RgbDescr<XpubDerivable>>::new_layer1(descr.clone(), Network::Regtest);
        w.addresses(0u8).next().unwrap().addr.script_pubkey()
    };
    let funding = Tx {
        version: TxVer::V2,
        inputs: Confined::from_checked(vec![TxIn {
            prev_output: Outpoint::new(Txid::from([0x11; 32]), 0),
            sig_script: none!(),
            sequence: SeqNo::from_consensus_u32(0xFFFF_FFFD),
            witness: Witness::default(),
        }]),
        outputs: Confined::from_checked(vec![TxOut::new(spk, Sats::from_sats(50_000u64))]),
        lock_time: LockTime::ZERO,
    };
    let funding_txid = funding.txid();
    let mut wallet = Wallet::new_layer1(descr, Network::Regtest);
    wallet.update(&Offline(funding)).into_result().expect("cache");

    let mut stock = Stock::in_memory();
    stock.import_kit(NonInflatableAsset::kit()).expect("NIA kit");
    let txid = rgbstd::Txid::from_str(&funding_txid.to_string()).unwrap();
    let seal = GenesisSeal::from(BlindSeal::with_blinding(txid, 0, 7));
    let contract = stock
        .contract_builder("ssi:rgbjs-test", NonInflatableAsset::schema().schema_id(), ChainNet::BitcoinRegtest)
        .unwrap()
        .add_global_state("spec", AssetSpec::new("DEMO", "Demo Asset", Precision::Indivisible))
        .unwrap()
        .add_global_state("terms", ContractTerms { text: RicardianContract::default(), media: None })
        .unwrap()
        .add_global_state("issuedSupply", Amount::from(1_000_000u64))
        .unwrap()
        .add_fungible_state("assetOwner", seal, 1_000_000u64)
        .unwrap()
        .issue_contract_raw(1_761_350_000)
        .expect("issue");
    let contract_id = contract.contract_id();
    contract.save_file(out.join("transfer-demo.contract.rgb")).unwrap();
    stock.import_contract(contract, NoResolver).unwrap();
    println!("funding {funding_txid}:0\ncontract {contract_id}");

    // Bob's blinded seal on an outpoint of his own; only its hash goes in the invoice.
    let bob = GraphSeal::new_random(rgbstd::Txid::from_str(&"33".repeat(32)).unwrap(), 0);
    let invoice = RgbInvoiceBuilder::new(XChainNet::bitcoin(
        rgbstd::bitcoin::Network::Regtest,
        Beneficiary::BlindedSeal(bob.to_secret_seal()),
    ))
    .set_contract(contract_id)
    .set_amount_raw(100u64)
    .set_assignment_name(strict_types::FieldName::from("assetOwner"))
    .finish();
    println!("invoice {invoice}");

    let mut wallet = RgbWallet::new(stock, wallet);
    let (psbt, _, transfer) = wallet
        .pay::<bpstd::psbt::PropKey, bpstd::psbt::Output>(&invoice, TransferParams::with(1000, 1000))
        .expect("pay");
    let _ = psbt;
    transfer.save_file(out.join("transfer-demo.transfer.rgb")).unwrap();
    println!("transfer saved");
}
