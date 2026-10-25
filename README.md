# rgbjs

JavaScript for both RGB lines, built from each line's own Rust crates compiled to WebAssembly.

**Early development, version 0.0.1. Not for real funds.** The API will change.

RGB is now two incompatible protocols:

| | v0.11 | v0.12 |
|---|---|---|
| Maintained by | RGB Protocol Association (rgb-protocol, RGB-Tools) | RGB-WG / LNP/BP |
| Crates used here | rgb-invoicing, rgb-ops, rgb-schemas 0.11.1 | rgb-std, rgb-invoice 0.12.0-rc.3 |
| Invoices start with | `rgb:` | `contract:` |
| Import | `rgbjs/v011` | `rgbjs/v012` |

The [RGB Docs](https://rgb-docs.github.io/getting-started/two-rgbs) explain the split.

rgbjs doesn't reimplement either protocol. It wraps the reference code, so it accepts and rejects what those crates accept and reject. Everything runs offline: rgbjs holds no keys, and its only network request is the opt-in `checkWitnesses` lookup.

## What works

| | v0.11 | v0.12 |
|---|---|---|
| Parse an invoice | yes | yes |
| Parse a contract ID | yes | yes |
| Tell which line a string or file is | yes | yes |
| Validate a consignment offline | yes (contracts and transfers) | yes (contracts and transfers) |
| Read contract state from a consignment | yes | yes |
| Issue a contract (test networks) | yes: NIA, CFA, UDA, IFA, PFA | yes: RGB20 FNA, RGB25 UFA, RGB21 NFT, or any issuer file |
| Create an invoice (test networks) | yes | yes |
| Check what a transfer pays you, from your invoice secret | yes | yes |
| Pay an invoice: unsigned PSBT plus the transfer file (test networks) | not yet | yes |
| Look up witness confirmations on an Esplora server (opt-in) | yes | yes |

## Use

```js
import { detectLine, parseInvoice, validateConsignment } from 'rgbjs'

detectLine('rgb:Tk3d0h5w-...')        // 'v0.11'
detectLine('contract:tb@...')         // 'v0.12'

const inv = parseInvoice('rgb:Tk3d0h5w-8v4XYCg-7e~Sc0o-Lu6rp~X-~Jt7VHS-jqgzFD8/~/BF/bcrt:utxob:MSLQCKkW-w6caphB-12do1nJ-HNfgxvv-WE3zOOC-s8yZyHq-Ihoa5')
inv.line         // 'v0.11'
inv.network      // 'regtest'
inv.contractId   // 'rgb:Tk3d0h5w-8v4XYCg-7e~Sc0o-Lu6rp~X-~Jt7VHS-jqgzFD8'
inv.amount       // '100' (a string, so large amounts stay exact)
inv.beneficiary  // { type: 'blinded-utxo', value: 'bcrt:utxob:...' }

const result = validateConsignment(new Uint8Array(bytes))  // a .rgb file from either line
result.valid       // true
result.contractId  // 'rgb:...' or 'contract:...'
result.witnesses   // the witness transactions it relies on
// v0.11 transfers: result.terminals lists the blinded seals the transfer is for;
//                  as the recipient, check that yours is there
// v0.11: result.schema       'NIA', 'CFA', 'UDA', 'IFA' or 'PFA'
//        result.state        { global: { spec: [...], issuedSupply: [...] }, owned: { assetOwner: [{ seal, value, witness, spent }] } }
// v0.12: result.issuer       { id, name, developer, trusted, ... }
//        result.state        { global: { ticker: ['"DEMO"'], ... }, owned: { balance: [{ outpoint, value }] } }
```

Creating, offline and on test networks only:

```js
import { v011, v012 } from 'rgbjs'

// v0.11: a NIA token on your output. `blinding` is a secret random u64: keep it.
const file = v011.issueContract({ schema: 'NIA', network: 'regtest', issuer: 'ssi:anonymous',
  name: 'Demo Token', ticker: 'DEMO', precision: 2, supply: '1000000', terms: '',
  timestamp: 1761400000, outpoint: 'txid:vout', blinding: '…' })   // RGB\0CON bytes

const { invoice, seal } = v011.createInvoice({ network: 'regtest', outpoint: 'txid:vout',
  blinding: '…', contractId: 'rgb:…', amount: '100' })

// v0.12: rgb-wallet's issue parameters as JSON, plus a secret noise seed (keep it).
const bytes = v012.issueContract(params, crypto.getRandomValues(new Uint8Array(32)))
const inv = v012.createInvoice({ contractId: 'contract:…', value: 100, outpoint: 'txid:vout', noiseSeed, nonce: 0 })
```

Did a transfer pay me? Give it the transfer and the secret from your invoice:

```js
const r = v011.receive(transferBytes, { outpoint, blinding })           // v0.12: { outpoint, noiseSeed, nonce }
r.receive.addressedToYou   // true if the transfer is for your seal
r.receive.received         // [{ name: 'assetOwner', value: '100', outpoint, witness }]
```

Paying (v0.12): give your history of the contract, your xpub and the coins you may spend. You get the witness transaction as an unsigned PSBT for your own signer, the transfer for the payee, and your updated file holding the change. Nothing is signed or broadcast:

```js
const p = v012.payInvoice({
  contracts: [backupBytes],                   // and any transfers you received
  invoice: 'contract:tb@…/100@at:…/',
  descriptor: '[fp/86h/1h/0h]tpub…/<0;1>/*',  // v012.deriveAddress(descriptor, '&0/0') gives its addresses
  coins: [{ outpoint: 'txid:vout', sats: 50000, terminal: '&0/0', tx: '<hex of the transaction that made it>' }],
  seals: [{ outpoint, nonce }],               // your invoices that were paid
  fee: 500, noiseSeed })                      // the 32-byte seed your contracts and invoices used
p.psbt       // base64; sign and broadcast it, then
p.transfer   // give this to the payee
p.backup     // keep this: pay from it next time (the change is p.change.vout of p.txid)
```

Are the witnesses mined? This is the only rgbjs call that uses the network, and only the transaction IDs are sent:

```js
import { checkWitnesses, ESPLORA } from 'rgbjs'
const c = await checkWitnesses(result.witnesses.map(w => w.txid), { server: ESPLORA.signet })
c.results   // [{ txid, state: 'mined' | 'mempool' | 'not-found', height, confirmations }]
```

[create.html](https://rgbjs.com/create.html) does this in the browser, and [check.html](https://rgbjs.com/check.html) checks any invoice or file.

Every function throws an `Error` with the upstream message when its input is invalid. To load only one line, import `rgbjs/v011` or `rgbjs/v012`.

### What `valid: true` means

Both lines run the reference validator. **Witness transactions aren't looked up on chain** in either: `valid: true` means valid *if* the listed witnesses confirm. Check them against your own node or indexer before you rely on a transfer.

**v0.11** runs `rgb-consensus`, with two limits:

- **Witnesses come from the consignment.** rgbjs uses the transactions carried in it.
- **Tested schemas:** all five have transfer vectors: NIA, CFA, UDA, IFA and PFA. PFA has a signed transfer and an unsigned one that must be rejected.
- **Only the standard schemas are trusted:** NIA, CFA, UDA, IFA and PFA from rgb-schemas 0.11.1 (`v011.knownSchemas()`). Other schemas are rejected. rgbjs is also stricter than rgb-consensus 0.11.1 here: each type in the consignment must match the trusted definition exactly, names included. Upstream compares types with a check that ignores field and variant names.

**v0.12** evaluates every operation into an in-memory contract, as `rgb accept -u` does. The contract ID commits to the codex and genesis, but not to the semantics (APIs, types, libraries) that say what the state means. Those come with the consignment, so check them:

- `issuer.trusted` is true only when the semantics match an issuer in `v012.knownIssuers()` exactly. Those are RGB20 FNA (RGB-WG examples), RGB21 UniqueNFT and RGB25 UniquelyFungible (pandora-prime/rgb-issuers), all of which have test vectors. For any other issuer, compare `issuer.id` with `v012.parseIssuer(file).id` for an issuer file you trust. The comparison is stricter than the issuer ID: it also covers type names, which the ID's checksum doesn't.
- Issuer signatures aren't checked. `signed` says whether one is present.
- rgbjs also rejects a consignment whose genesis names a different codex from the one the contract uses. Upstream doesn't check this field.

## Known issues

- `wasm/v012` is built with patched copies of `strict_encoding` 2.9.1 and `strict_types` 2.9.1 (`rust/patches/`). Both pre-allocate memory from lengths read out of the data. One corrupted byte in a consignment can then request ~4.4 GB, or overflow capacity and panic. The pinned `rgb-wallet` 0.12.0-rc.3 crashes on `rgb accept` of such a file. The patches cap pre-allocation, as rgb-protocol's `rgb-strict-encoding` 1.0.5 already does for v0.11. The tests check that no corrupted consignment traps.

- v0.12 invoices print as `contract:tb@contract:<id>/...` instead of `contract:tb@<id>/...` ([AluVM/sonic#27](https://github.com/AluVM/sonic/issues/27)). The cause is that `CallScope`'s `Display` doesn't pass the `{:-}` flag on to `ContractId`. Both forms parse to the same invoice.
- The 0.11.2 consignment format (rgb-api 0.11.2-rc.1) can't be read. 0.11.1 is pinned until 0.11.2 is final.
- Size: `wasm/v011` is 2.9 MB (680 KB gzipped) and `wasm/v012` 1.7 MB (420 KB gzipped). secp256k1 is built with `lowmemory` to keep its precomputed tables small, which makes signature checks a little slower. `rgb-consensus` 0.11.1 still pulls in two secp256k1 versions.
- The transfer vectors come from `rust/gen-v011` and `rust/gen-v012`, which drive `rgb-api` 0.11.1 and `rgb-runtime` 0.12.0-rc.3 with a made-up funding transaction instead of an indexer. The pinned `rgb-wallet` accepts the v0.12 one. The pinned `rgb-cmd` 0.11.1 can't check the v0.11 one offline, because its `validate` and `accept` look witnesses up on chain.

## Tests

The rules rgbjs is tested against are written up as a specification at [rgb-conformance](https://rgbjs.github.io/rgb-conformance/): 41 rules for both lines as JSON-LD, each with vectors, and a live suite.

```sh
npm test
```

The tests run against vectors in `test/vectors/`:

- **Upstream invoice tests.** `scripts/extract-vectors.py` extracts every case from the invoice tests in `rgb-invoicing` 0.11.1 and `sonic-callreq` 0.12.0, so rgbjs must agree with the upstream test suites.
- **CLI invoices.** Real invoices printed by `rgb-cmd` 0.11.1 and `rgb-wallet` 0.12.0-rc.3, with the inputs that produced them.
- **A transfer for each line.** Made offline by `rust/gen-v011` and `rust/gen-v012`. The pinned `rgb-wallet` accepts the v0.12 one.
- **Contract consignments from both CLIs.** Exported by `rgb-cmd` 0.11.1 and `rgb-wallet` 0.12.0-rc.3. Each is checked as valid. Every single-byte corruption of each is checked too: it must be rejected, or come out as a different contract, an untrusted schema or issuer, or a different state.

## Build

The `wasm/` folder is committed, so `npm test` needs only Node 18 or later. To rebuild it:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129
npm run build
```

`rust/v011` and `rust/v012` are separate crates with their own `Cargo.lock`, because the two lines pin incompatible versions of shared dependencies.

## License

MIT for rgbjs's own code. The compiled wasm includes Apache-2.0 crates; see [NOTICE](NOTICE).
