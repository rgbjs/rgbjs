# rgbjs

JavaScript for both RGB lines, built from each line's own Rust crates compiled to WebAssembly.

**Early development, version 0.0.1. Not for real funds.** The API will change.

RGB is now two incompatible protocols:

| | v0.11 | v0.12 |
|---|---|---|
| Maintained by | RGB Protocol Association (rgb-protocol, RGB-Tools) | RGB-WG / LNP/BP |
| Crates used here | rgb-invoicing, rgb-ops, rgb-schemas 0.11.1 | rgb-invoice 0.12.0-rc.3, sonic-callreq 0.12.0 |
| Invoices start with | `rgb:` | `contract:` |
| Import | `rgbjs/v011` | `rgbjs/v012` |

The [RGB Docs](https://rgb-docs.github.io/getting-started/two-rgbs) explain the split.

rgbjs doesn't reimplement either protocol. It wraps the reference code, so it accepts and rejects what those crates accept and reject. Everything runs offline: rgbjs holds no keys and makes no network requests.

## What works

| | v0.11 | v0.12 |
|---|---|---|
| Parse an invoice | yes | yes |
| Parse a contract ID | yes | yes |
| Tell which line a string or file is | yes | yes |
| Validate a consignment offline | yes (contracts; transfers untested, see below) | not yet |

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

const result = validateConsignment(new Uint8Array(bytes))  // a v0.11 .rgb file
result.valid       // true
result.contractId  // 'rgb:...'
result.schema      // 'NIA'
result.witnesses   // [{ txid, included }]
```

Every function throws an `Error` with the upstream message when its input is invalid. To load only one line, import `rgbjs/v011` or `rgbjs/v012`.

### What `valid: true` means

v0.11 validation runs the full `rgb-consensus` validator, with two limits:

- **Witness transactions aren't looked up on chain.** rgbjs uses the transactions carried in the consignment and doesn't check that they are mined. `valid: true` means valid *if* the listed witnesses confirm. Check them against your own node or indexer before you rely on a transfer.
- **Only the standard schemas are trusted:** NIA, CFA, UDA, IFA and PFA from rgb-schemas 0.11.1 (`v011.knownSchemas()`). Other schemas are rejected. rgbjs is also stricter than rgb-consensus 0.11.1 here: each type in the consignment must match the trusted definition exactly, names included. Upstream compares types with a check that ignores field and variant names.

## Known issues

- v0.12 invoices print as `contract:tb@contract:<id>/...` instead of `contract:tb@<id>/...` ([AluVM/sonic#27](https://github.com/AluVM/sonic/issues/27)). The cause is that `CallScope`'s `Display` doesn't pass the `{:-}` flag on to `ContractId`. Both forms parse to the same invoice.
- The 0.11.2 consignment format (rgb-api 0.11.2-rc.1) can't be read. 0.11.1 is pinned until 0.11.2 is final.
- `wasm/v011` is about 4.9 MB (2.8 MB gzipped). Most of that is two copies of secp256k1 pulled in by upstream dependencies.
- No transfer consignment in 0.11.1 format is in the test vectors yet. Making one needs a regtest chain.

## Tests

```sh
npm test
```

The tests run against vectors in `test/vectors/`:

- **Upstream invoice tests.** `scripts/extract-vectors.py` extracts every case from the invoice tests in `rgb-invoicing` 0.11.1 and `sonic-callreq` 0.12.0, so rgbjs must agree with the upstream test suites.
- **CLI invoices.** Real invoices printed by `rgb-cmd` 0.11.1 and `rgb-wallet` 0.12.0-rc.3, with the inputs that produced them.
- **A v0.11 contract consignment.** Exported by `rgb-cmd` 0.11.1. It's checked as valid, and every single-byte corruption of it is checked as rejected.

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
