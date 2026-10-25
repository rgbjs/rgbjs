// RGB v0.12 (RGB-WG / LNP/BP crates 0.12.0-rc.3).
import init, * as wasm from '../wasm/v012/rgbjs_v012.js'
import { call, load } from './load.js'

await load(init, new URL('../wasm/v012/rgbjs_v012_bg.wasm', import.meta.url))

export const line = 'v0.12'

/** Parses a `contract:` invoice. Throws on anything that is not a v0.12 invoice. */
export const parseInvoice = invoice => call(wasm.parseInvoice, invoice)

/** Parses a `contract:` contract ID. */
export const parseContractId = id => call(wasm.parseContractId, id)

/**
 * Validates a consignment file (`RGBCNSGN`, from `rgb backup`, `consign` or
 * `pay`) offline by evaluating every operation into an in-memory contract,
 * as `rgb accept -u` does. Witnesses are not looked up on chain, and issuer
 * signatures are not checked (`signed` says whether one is present).
 *
 * The contract ID commits to the codex and genesis but not to the semantics
 * (APIs, types, libraries) that say what the state means; those travel in
 * the consignment. `issuer.trusted` is true only when they match an issuer
 * in `knownIssuers()` exactly. For other issuers, compare `issuer.id` with
 * `parseIssuer(file).id` for an issuer file you trust.
 */
export const validateConsignment = bytes => call(wasm.validateConsignment, bytes)

/** Parses an `.issuer` file; its `id` is comparable with `issuer.id` above. */
export const parseIssuer = bytes => call(wasm.parseIssuer, bytes)

/** The issuers whose semantics `validateConsignment` trusts out of the box. */
export const knownIssuers = () => call(wasm.knownIssuers)

/**
 * Issues a contract on a test network and returns its `RGBCNSGN` file bytes,
 * as `rgb backup` writes. `params` has the shape of rgb-wallet's issue
 * parameter file; `noiseSeed` is secret random bytes (keep them) that blind
 * the seals; `issuerFile` is an optional `.issuer` for an unknown issuer.
 */
export function issueContract (params, noiseSeed, issuerFile) {
  try { return wasm.issueContract(JSON.stringify(params), noiseSeed, issuerFile) } catch (err) { throw err instanceof Error ? err : new Error(String(err)) }
}

/**
 * Creates an invoice paying to an auth token on one of your outputs, derived
 * from `outpoint`, the secret `noiseSeed` and `nonce` as rgb-wallet does.
 * Returns { invoice, authToken, seal }.
 */
export const createInvoice = ({ contractId, value, outpoint, noiseSeed, nonce = 0 }) =>
  call(wasm.createInvoice, contractId, String(value), outpoint, noiseSeed, BigInt(nonce))

/**
 * Did this consignment pay me? Give the file and your invoice secret
 * ({ outpoint, noiseSeed, nonce }). Returns the validation result plus
 * `receive: { authToken, addressedToYou, received: [{ name, value, outpoint, witness }] }`.
 */
export const receive = (bytes, { outpoint, noiseSeed, nonce = 0 }) => call(wasm.receive, bytes, outpoint, noiseSeed, BigInt(nonce))

/**
 * Pays an invoice and returns { psbt, txid, tx, fee, change, transfer,
 * backup }: the witness transaction as an unsigned PSBT (base64, version 0)
 * for your own signer, the transfer file (bytes) to give the payee once it
 * is broadcast, and your own updated contract file (bytes), which holds the
 * change: pay from it next time. Nothing is signed or sent.
 *
 * `contracts` is the contract's history as you hold it (the issuer's backup
 * and any transfers you received): one file's bytes or a list. `descriptor`
 * is your BIP86 taproot xpub with origin and `/<0;1>/*`; `coins` are the
 * outputs you may spend, [{ outpoint, sats, terminal, tx }] with the hex of
 * the transaction that made each; `seals` are your invoices' { outpoint,
 * nonce }; `noiseSeed` is the 32-byte secret they were made with. `fee` is
 * in sats. rgb-wallet 0.12.0-rc.3 always commits with an OP_RETURN output.
 */
export function payInvoice ({ contracts, invoice, descriptor, coins, seals = [], fee, giveaway, noiseSeed }) {
  const files = Array.isArray(contracts) ? contracts : [contracts]
  const all = new Uint8Array(files.reduce((n, f) => n + 4 + f.length, 0))
  let at = 0
  for (const f of files) { new DataView(all.buffer).setUint32(at, f.length); all.set(f, at + 4); at += 4 + f.length }
  const r = call(wasm.payInvoice, all, JSON.stringify({ descriptor, coins, seals, invoice, fee, giveaway }), noiseSeed)
  const bytes = h => Uint8Array.from(h.match(/../g), x => parseInt(x, 16))
  r.transfer = bytes(r.transfer)
  r.backup = bytes(r.backup)
  return r
}

/** The testnet address and scriptPubKey (hex) of `descriptor` at `terminal`, such as `&0/0`. */
export const deriveAddress = (descriptor, terminal = '&0/0') => call(wasm.deriveAddress, descriptor, terminal)
