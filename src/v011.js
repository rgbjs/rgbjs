// RGB v0.11 (RGB Protocol Association / rgb-protocol crates 0.11.1).
import init, * as wasm from '../wasm/v011/rgbjs_v011.js'
import { call, load } from './load.js'

await load(init, new URL('../wasm/v011/rgbjs_v011_bg.wasm', import.meta.url))

export const line = 'v0.11'

/** Parses an `rgb:` invoice. Throws on anything that is not a v0.11 invoice. */
export const parseInvoice = invoice => call(wasm.parseInvoice, invoice)

/** Parses an `rgb:` contract ID. */
export const parseContractId = id => call(wasm.parseContractId, id)

/** The standard schemas (rgb-schemas 0.11.1) whose types validation trusts. */
export const knownSchemas = () => call(wasm.knownSchemas)

/**
 * Validates a binary consignment (`RGB\0TFR` transfer or `RGB\0CON` contract)
 * offline. Witness transactions come from the consignment and are not looked
 * up on chain: `valid: true` means valid if the listed witnesses confirm.
 */
export const validateConsignment = bytes => call(wasm.validateConsignment, bytes)

/**
 * Issues a contract on a test network and returns its `RGB\0CON` file bytes.
 * `params`: { schema: 'NIA'|'CFA'|'UDA'|'IFA'|'PFA', network, issuer, name,
 * ticker, precision, supply, terms, timestamp, outpoint: 'txid:vout',
 * blinding } plus maxSupply/inflationOutpoint/inflationBlinding (IFA),
 * pubkey (PFA) or tokenIndex (UDA). Keep `blinding` secret.
 */
export function issueContract (params) {
  try { return wasm.issueContract(JSON.stringify(params)) } catch (err) { throw err instanceof Error ? err : new Error(String(err)) }
}

/**
 * Creates an invoice paying to a blinded seal on one of your outputs.
 * `params`: { network, outpoint: 'txid:vout', blinding, contractId?,
 * amount? | tokenIndex?, assignmentName?, expiry?, endpoints? }. Returns
 * { invoice, secretSeal, seal }: keep `seal` to claim what arrives.
 */
export const createInvoice = params => call(wasm.createInvoice, JSON.stringify(params))

/**
 * Did this transfer pay me? Give the transfer file and the `seal` from your
 * invoice ({ outpoint, blinding }). Returns the validation result plus
 * `receive: { secretSeal, addressedToYou, received: [{ name, value, outpoint, witness }] }`.
 */
export const receive = (bytes, { outpoint, blinding }) => call(wasm.receive, bytes, outpoint, String(blinding))
