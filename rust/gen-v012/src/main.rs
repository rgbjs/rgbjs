//! Makes a v0.12 transfer consignment without any indexer or network.
//!
//!   gen-v012 <data dir copied from RGB-WG/rgb v0.12.0-rc.3 examples/data> <DemoToken.yaml> <out dir>
//!
//! Alice's demo wallet gets a made-up funding transaction (never broadcast),
//! a contract is issued on its output, and Alice pays Bob 10 units. The
//! resolver only knows that one transaction and refuses to broadcast. The
//! witness transaction is left unsigned: client-side validation doesn't need
//! signatures.

#[macro_use]
extern crate amplify;

use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::str::FromStr;

use amplify::confinement::Confined;
use bpstd::psbt::{TxParams, Utxo};
use bpstd::seals::TxoSeal;
use bpstd::{
    LockTime, Network, Outpoint, Sats, ScriptPubkey, SeqNo, Terminal, Tx, TxIn, TxOut, TxVer, Txid,
    UnsignedTx, Witness, Derive,
};
use rgb::invoice::RgbInvoice;
use rgb::popls::bp::RgbWallet;
use rgb::{Consensus, ContractId, Contracts, CreateParams, EitherSeal, WitnessStatus};
use rgb_persist_fs::StockpileDir;
use rgbp::resolvers::{Resolver, ResolverError};
use rgbp::{CoinselectStrategy, FileHolder, Owner, OwnerProvider, RgbRuntime, UtxoSet};

struct Offline(HashMap<Txid, UnsignedTx>);

impl Resolver for Offline {
    fn resolve_tx(&self, txid: Txid) -> Result<Option<UnsignedTx>, ResolverError> {
        Ok(self.0.get(&txid).cloned())
    }
    fn resolve_tx_status(&self, _: Txid) -> Result<WitnessStatus, ResolverError> {
        Ok(WitnessStatus::Tentative)
    }
    fn resolve_utxos(
        &self,
        _: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>> {
        std::iter::empty()
    }
    fn last_block_height(&self) -> Result<u64, ResolverError> { Ok(0) }
    fn broadcast(&self, _: &Tx) -> Result<(), ResolverError> {
        panic!("gen-v012 never broadcasts")
    }
}

fn main() {
    let mut args = std::env::args().skip(1).map(PathBuf::from);
    let (data, params, out) = (args.next().unwrap(), args.next().unwrap(), args.next().unwrap());
    let dir = data.join("bitcoin.testnet");

    let mut holder = FileHolder::load(dir.join("alice.wallet")).expect("alice wallet");
    let terminal = Terminal::from_str("&0/1").unwrap();
    let spk = holder.descriptor().derive(terminal.keychain, terminal.index).next().unwrap().to_script_pubkey();

    // A made-up funding transaction paying Alice; it exists only here.
    let funding = Tx {
        version: TxVer::V2,
        inputs: Confined::from_checked(vec![TxIn {
            prev_output: Outpoint::new(Txid::from([0x11; 32]), 0),
            sig_script: none!(),
            sequence: SeqNo::from_consensus_u32(0xFFFF_FFFD),
            witness: Witness::default(),
        }])
        ,
        outputs: Confined::from_checked(vec![TxOut::new(spk, Sats::from_sats(50_000u64))]),
        lock_time: LockTime::ZERO,
    };
    let funding_out = Outpoint::new(funding.txid(), 0);
    let utxos = holder.utxos_mut();
    for op in utxos.outpoints().collect::<Vec<_>>() {
        utxos.remove(op);
    }
    utxos.insert(funding_out, Sats::from_sats(50_000u64), terminal);

    let mut params: CreateParams<Outpoint> =
        serde_yaml::from_reader(File::open(params).unwrap()).expect("DemoToken.yaml");
    params.name = "TransferDemo".into();
    for owned in &mut params.owned {
        owned.state.seal = EitherSeal::Alt(funding_out);
    }

    let stockpile = StockpileDir::<TxoSeal>::load(dir.clone(), Consensus::Bitcoin, true).unwrap();
    let resolver = Offline(HashMap::from([(funding.txid(), UnsignedTx::from(funding.clone()))]));
    let wallet = Owner::with_components(Network::Testnet4, holder, resolver);
    let contracts: Contracts<StockpileDir<TxoSeal>> = Contracts::load(stockpile);
    let mut runtime = RgbRuntime::from(RgbWallet::with_components(wallet, contracts));

    let contract_id: ContractId = runtime.issue(params).expect("issue");
    println!("funding {funding_out}\ncontract {contract_id}");
    runtime.contracts.export_to_file(out.join("TransferDemo.contract.rgb"), contract_id).unwrap();

    let auth = std::env::var("BOB_AUTH").expect("BOB_AUTH=<at:... from `rgb invoice --seal-only`>");
    let invoice = format!("contract:tb@{contract_id:-}/10@{auth}/");
    let invoice = RgbInvoice::<ContractId>::from_str(&invoice).expect("invoice");
    let (psbt, payment) = runtime
        .pay_invoice(&invoice, CoinselectStrategy::Aggregate, TxParams::with(Sats::from_sats(1000u64)), None)
        .expect("pay");
    println!("witness {}", psbt.txid());
    runtime
        .contracts
        .consign_to_file(out.join("TransferDemo.transfer.rgb"), contract_id, payment.terminals)
        .unwrap();
}
