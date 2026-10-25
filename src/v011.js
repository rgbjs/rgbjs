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
