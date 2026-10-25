# rgbjs

JavaScript for both RGB lines, built from each line's own Rust crates compiled to WebAssembly.

**Early development, version 0.0.2. Not for real funds.** The API will change.

RGB is now two incompatible protocols:

| | v0.11 | v0.12 |
|---|---|---|
| Maintained by | RGB Protocol Association (rgb-protocol, RGB-Tools) | RGB-WG / LNP/BP |
| Crates used here | rgb-invoicing, rgb-ops, rgb-schemas 0.11.1 | rgb-std, rgb-invoice, rgb-runtime 0.12.0-rc.3 |
| Invoices start with | `rgb:` | `contract:` |
| Import | `rgbjs/v011` | `rgbjs/v012` |

The [RGB Docs](https://rgb-docs.github.io/getting-started/two-rgbs) explain the split.

Used by [Bunting](https://github.com/bitcoin-blake/bunting), a browser-tab RGB wallet on the Bitcoin BLAKE2b testnet4 ([how RGB runs there](https://rgb-docs.github.io/guides/development/rgb-on-blake/)).

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
const inv = v012.createInvoice({ contractId: 'contract:…', amount: 100, outpoint: 'txid:vout', noiseSeed, nonce: 0, expiry })  // expiry: optional, Unix seconds
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
  fee: 500, noiseSeed,                        // the 32-byte seed your contracts and invoices used
  commit: 'tapret' })                         // or 'opret' (the default): see below
p.psbt       // base64 (PSBT version 0); sign and broadcast it, then
p.transfer   // bytes: give this to the payee
p.backup     // bytes: keep this, and pay from it next time
p.txid       // the witness transaction's ID; p.tx is the unsigned transaction as hex
p.change     // { vout, terminal } of your change output, or null
p.tapret     // with commit: 'tapret', { vout, merkleRoot, scriptPubkey } (hex); otherwise null
p.fee        // in sats
```

The commitment goes in an OP_RETURN output (`opret`, what rgb-wallet 0.12.0-rc.3 always makes), or with `commit: 'tapret'` inside the change output's taproot key, so the transaction has no OP_RETURN and its change looks like any taproot output. Nodes that limit OP_RETURN data don't affect tapret. A tapret change coin is at `p.tapret.scriptPubkey`, not the descriptor's plain address: to spend it later, sign with the key tweaked by `p.tapret.merkleRoot` (BIP341).

Are the witnesses mined? This is the only rgbjs call that uses the network, and only the transaction IDs are sent:

```js
import { checkWitnesses, ESPLORA } from 'rgbjs'
const c = await checkWitnesses(result.witnesses.map(w => w.txid), { server: ESPLORA.signet })  // or ESPLORA.blake for txbt4
c.results   // [{ txid, state: 'mined' | 'mempool' | 'not-found', height, confirmations }]
```

[create.html](https://rgbjs.com/create.html) does this in the browser, and [check.html](https://rgbjs.com/check.html) checks any invoice or file.

Every function throws an `Error` when its input is invalid, saying what the input wasn't ("not a valid v0.12 invoice: …") and then the upstream reason. To load only one line, import `rgbjs/v011` or `rgbjs/v012`.

### API

| Export | From | Takes | Returns |
|---|---|---|---|
| `detectLine` | `rgbjs` | a string or `Uint8Array` | `'v0.11'`, `'v0.12'` or `null` |
| `parseInvoice` | `rgbjs`, `v011`, `v012` | an invoice string | the invoice's fields, with `line` |
| `parseContractId` | `rgbjs`, `v011`, `v012` | a contract ID | `{ line, contractId, … }` in canonical form |
| `validateConsignment` | `rgbjs`, `v011`, `v012` | a `.rgb` file (`Uint8Array`) | `{ valid, error?, contractId, state, witnesses, … }` |
| `assetSummary` | `rgbjs` | a `validateConsignment` result | `{ ticker, name, decimals, issued }` |
| `formatAmount` | `rgbjs` | a raw amount string, decimals | the amount with its decimal point |
| `checkWitnesses` | `rgbjs` | txids, `{ server, fetch? }` | `{ server, tip, results: [{ txid, state, height, confirmations }] }` (network) |
| `defaultServer` | `rgbjs` | a `validateConsignment` result | the public Esplora URL for its network, or `null` |
| `ESPLORA` | `rgbjs` | | public Esplora URLs: `bitcoin`, `testnet4`, `testnet3`, `signet`, `blake` |
| `v011`, `v012` | `rgbjs` | | each line's module, as below |
| `line` | `v011`, `v012` | | `'v0.11'` or `'v0.12'` |
| `knownSchemas` | `v011` | | the schemas rgbjs trusts |
| `knownIssuers` | `v012` | | the issuers rgbjs trusts |
| `parseIssuer` | `v012` | an `.issuer` file (`Uint8Array`) | `{ id, name, … }` |
| `issueContract` | `v011` | `{ schema, network, … }` | a contract file (`Uint8Array`) |
| `issueContract` | `v012` | rgb-wallet's issue parameters, a noise seed, an optional `.issuer` file | a contract file (`Uint8Array`) |
| `createInvoice` | `v011` | `{ network, outpoint, blinding, contractId?, amount?, … }` | `{ invoice, secretSeal, seal }` |
| `createInvoice` | `v012` | `{ contractId, amount, outpoint, noiseSeed, nonce?, expiry? }` | `{ invoice, authToken, seal }` |
| `receive` | `v011` | a transfer, `{ outpoint, blinding }` | the validation result plus `receive: { addressedToYou, received }` |
| `receive` | `v012` | a transfer, `{ outpoint, noiseSeed, nonce? }` | the validation result plus `receive: { authToken, addressedToYou, received }` |
| `payInvoice` | `v012` | `{ contracts, invoice, descriptor, coins, seals?, fee, noiseSeed, giveaway?, commit? }` | `{ line, psbt, txid, tx, fee, change, tapret, transfer, backup }` |
| `deriveAddress` | `v012` | an xpub descriptor, a terminal such as `'&0/0'` | `{ address, scriptPubkey }` |

Creating and paying work on test networks only: mainnet parameters and invoices are refused.

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
- v0.12 invoice expiry: upstream (`sonic-callreq` 0.12.0) writes `?expiry=` as RFC 3339 with a `+00:00` offset, and its own parser rejects that form (the `+` reads as a space). `v012.createInvoice({ expiry })` writes UTC with a `Z`, which reads back. Upstream also writes several query parameters without `&` between them.
- The 0.11.2 consignment format (rgb-api 0.11.2-rc.1) can't be read. 0.11.1 is pinned until 0.11.2 is final.
- Size: `wasm/v011` is 3.4 MB (745 KB gzipped) and `wasm/v012` 2.7 MB (680 KB gzipped, of which rgb-runtime for payments is about 570 KB raw). secp256k1 is built with `lowmemory` to keep its precomputed tables small, which makes signature checks a little slower. `rgb-consensus` 0.11.1 still pulls in two secp256k1 versions.
- rgb-runtime 0.12.0-rc.3 always commits a payment with an `OP_RETURN` output. `payInvoice({ commit: 'tapret' })` runs its payment steps (fulfill, compose, resolve, bundle, fill, complete) itself, with the change output marked as the tapret host instead. The pinned `rgb-wallet` accepts the result but can't make one.
- When a later payment spends a tapret change coin, the PSBT records that input's plain (untweaked) script, because rgb-runtime's `Owner::utxo` takes the first script the descriptor derives. The transaction is right; sign it with the script and tweak you saved from `p.tapret`, not the PSBT's `witness_utxo`.
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
- **Payments made by rgbjs.** `test/pay.test.js` issues, pays (opret and tapret), pays again from the change and onward from what was received, through a five-payment chain, checking every file validates and pays its payee. `paid-fna` and `paid-tapret` are sample payments; the pinned `rgb-wallet` accepts both.
- **Payments mined on a test network.** `mined-opret`, `mined-tapret` and `mined-to-bob` are real payments on the Bitcoin BLAKE2b testnet4 (blocks 153,561, 153,562 and 153,567), made with `payInvoice` and signed by [Bunting](https://bitcoin-blake.github.io/bunting/). See `test/vectors/v012/README.md` for their witnesses.
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
