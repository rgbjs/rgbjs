//! In-memory `Stock` and `Pile`, ported from `sonic-persist-fs` 0.12.0 and
//! `rgb-persist-fs` 0.12.0-rc.3 (Apache-2.0), which only exist backed by files.
//! Used to evaluate a consignment without touching any storage.

use std::collections::{BTreeMap, BTreeSet};
use std::marker::PhantomData;

use amplify::confinement::SmallOrdMap;
use amplify::MultiError;
use hypersonic::{
    Articles, CellAddr, EffectiveState, Operation, Opid, SemanticError, Stock, Transition,
};
use bp::seals::TxoSeal;
use rgb::{OpRels, Pile, RgbSeal, Witness, WitnessStatus};

type Seal = TxoSeal;

/// Returned by `load`: an in-memory store has nothing to load from.
#[derive(Debug, Display, Error)]
#[display("in-memory storage can't be loaded")]
pub struct NotPersistent;

#[derive(Debug)]
pub struct StockMem {
    stash: BTreeMap<Opid, Operation>,
    trace: BTreeMap<Opid, Transition>,
    valid: BTreeMap<Opid, bool>,
    spent: BTreeMap<CellAddr, Opid>,
    read: BTreeMap<CellAddr, Vec<Opid>>,
    articles: Articles,
    state: EffectiveState,
}

impl Stock for StockMem {
    type Conf = ();
    type Error = NotPersistent;

    fn new(articles: Articles, state: EffectiveState, _: ()) -> Result<Self, NotPersistent> {
        Ok(Self {
            stash: none!(),
            trace: none!(),
            valid: none!(),
            spent: none!(),
            read: none!(),
            articles,
            state,
        })
    }

    fn load(_: ()) -> Result<Self, NotPersistent> { Err(NotPersistent) }

    fn config(&self) -> Self::Conf {}

    fn articles(&self) -> &Articles { &self.articles }
    fn state(&self) -> &EffectiveState { &self.state }

    fn is_valid(&self, opid: Opid) -> bool { self.valid.get(&opid).copied().unwrap_or_default() }
    fn mark_valid(&mut self, opid: Opid) { self.valid.insert(opid, true); }
    fn mark_invalid(&mut self, opid: Opid) { self.valid.insert(opid, false); }

    fn has_operation(&self, opid: Opid) -> bool { self.stash.contains_key(&opid) }
    fn operation_count(&self) -> u64 { self.stash.len() as u64 }
    fn operation(&self, opid: Opid) -> Operation { self.stash[&opid].clone() }
    fn operations(&self) -> impl Iterator<Item = (Opid, Operation)> {
        self.stash.iter().map(|(k, v)| (*k, v.clone()))
    }
    fn transition(&self, opid: Opid) -> Transition { self.trace[&opid].clone() }
    fn trace(&self) -> impl Iterator<Item = (Opid, Transition)> {
        self.trace.iter().map(|(k, v)| (*k, v.clone()))
    }
    fn read_by(&self, addr: CellAddr) -> impl Iterator<Item = Opid> {
        self.read.get(&addr).cloned().unwrap_or_default().into_iter()
    }
    fn spent_by(&self, addr: CellAddr) -> Option<Opid> { self.spent.get(&addr).copied() }

    fn update_articles(
        &mut self,
        f: impl FnOnce(&mut Articles) -> Result<bool, SemanticError>,
    ) -> Result<bool, MultiError<SemanticError, NotPersistent>> {
        f(&mut self.articles).map_err(MultiError::A)
    }

    fn update_state<R>(
        &mut self,
        f: impl FnOnce(&mut EffectiveState, &Articles) -> R,
    ) -> Result<R, NotPersistent> {
        let res = f(&mut self.state, &self.articles);
        self.state.recompute(self.articles.semantics());
        Ok(res)
    }

    fn add_operation(&mut self, opid: Opid, operation: &Operation) {
        self.stash.insert(opid, operation.clone());
    }
    fn add_transition(&mut self, opid: Opid, transition: &Transition) {
        self.trace.insert(opid, transition.clone());
    }
    fn add_reading(&mut self, addr: CellAddr, reader: Opid) {
        self.read.entry(addr).or_default().push(reader);
    }
    fn add_spending(&mut self, spent: CellAddr, spender: Opid) { self.spent.insert(spent, spender); }
    fn commit_transaction(&mut self) {}
}

pub struct PileMem {
    hoard: BTreeMap<<Seal as RgbSeal>::WitnessId, <Seal as RgbSeal>::Client>,
    cache: BTreeMap<<Seal as RgbSeal>::WitnessId, <Seal as RgbSeal>::Published>,
    keep: BTreeMap<CellAddr, <Seal as RgbSeal>::Definition>,
    index: BTreeMap<Opid, BTreeSet<<Seal as RgbSeal>::WitnessId>>,
    stand: BTreeMap<<Seal as RgbSeal>::WitnessId, BTreeSet<Opid>>,
    mine: BTreeMap<<Seal as RgbSeal>::WitnessId, WitnessStatus>,
}

impl Pile for PileMem {
    type Seal = Seal;
    type Conf = ();
    type Error = NotPersistent;

    fn new(_: ()) -> Result<Self, NotPersistent> {
        Ok(Self {
            hoard: none!(),
            cache: none!(),
            keep: none!(),
            index: none!(),
            stand: none!(),
            mine: none!(),
        })
    }

    fn load(_: ()) -> Result<Self, NotPersistent> { Err(NotPersistent) }

    fn has_witness(&self, wid: <Seal as RgbSeal>::WitnessId) -> bool { self.hoard.contains_key(&wid) }
    fn pub_witness(&self, wid: <Seal as RgbSeal>::WitnessId) -> <Seal as RgbSeal>::Published { self.cache[&wid].clone() }
    fn cli_witness(&self, wid: <Seal as RgbSeal>::WitnessId) -> <Seal as RgbSeal>::Client { self.hoard[&wid].clone() }
    fn witness_status(&self, wid: <Seal as RgbSeal>::WitnessId) -> WitnessStatus { self.mine[&wid] }

    fn witness_ids(&self) -> impl Iterator<Item = <Seal as RgbSeal>::WitnessId> {
        self.stand.keys().copied().collect::<Vec<_>>().into_iter()
    }

    fn witnesses(&self) -> impl Iterator<Item = Witness<Seal>> {
        self.hoard.iter().map(|(wid, client)| Witness {
            id: *wid,
            published: self.cache[wid].clone(),
            client: client.clone(),
            status: self.mine[wid],
            opids: self.stand.get(wid).cloned().unwrap_or_default().into_iter().collect(),
        })
    }

    fn op_witness_ids(&self, opid: Opid) -> impl ExactSizeIterator<Item = <Seal as RgbSeal>::WitnessId> {
        self.index.get(&opid).cloned().unwrap_or_default().into_iter().collect::<Vec<_>>().into_iter()
    }

    fn ops_by_witness_id(&self, wid: <Seal as RgbSeal>::WitnessId) -> impl ExactSizeIterator<Item = Opid> {
        self.stand.get(&wid).cloned().unwrap_or_default().into_iter().collect::<Vec<_>>().into_iter()
    }

    fn seal(&self, addr: CellAddr) -> Option<<Seal as RgbSeal>::Definition> { self.keep.get(&addr).cloned() }

    fn seals(&self, opid: Opid, up_to: u16) -> SmallOrdMap<u16, <Seal as RgbSeal>::Definition> {
        let mut seals = SmallOrdMap::new();
        for no in 0..up_to {
            if let Some(seal) = self.keep.get(&CellAddr::new(opid, no)) {
                let _ = seals.insert(no, seal.clone());
            }
        }
        seals
    }

    fn op_relations(&self, opid: Opid, up_to: u16) -> OpRels<Seal> {
        let defines = self.seals(opid, up_to);
        let witness_ids = self.index.get(&opid).cloned().unwrap_or_default().into_iter().collect();
        OpRels { opid, witness_ids, defines, _phantom: PhantomData }
    }

    fn add_witness(
        &mut self,
        opid: Opid,
        wid: <Seal as RgbSeal>::WitnessId,
        published: &<Seal as RgbSeal>::Published,
        anchor: &<Seal as RgbSeal>::Client,
        status: WitnessStatus,
    ) {
        self.index.entry(opid).or_default().insert(wid);
        self.stand.entry(wid).or_default().insert(opid);
        self.hoard.insert(wid, anchor.clone());
        self.cache.insert(wid, published.clone());
        self.mine.entry(wid).or_insert(status);
    }

    fn add_seals(&mut self, opid: Opid, seals: SmallOrdMap<u16, <Seal as RgbSeal>::Definition>) {
        for (no, seal) in seals {
            self.keep.insert(CellAddr::new(opid, no), seal);
        }
    }

    fn update_witness_status(&mut self, wid: <Seal as RgbSeal>::WitnessId, status: WitnessStatus) {
        self.mine.insert(wid, status);
    }

    fn commit_transaction(&mut self) {}
}
