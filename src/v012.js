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
