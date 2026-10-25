// RGB v0.12 (RGB-WG / LNP/BP crates 0.12.0-rc.3).
import init, * as wasm from '../wasm/v012/rgbjs_v012.js'
import { call, load } from './load.js'

await load(init, new URL('../wasm/v012/rgbjs_v012_bg.wasm', import.meta.url))

export const line = 'v0.12'

/** Parses a `contract:` invoice. Throws on anything that is not a v0.12 invoice. */
export const parseInvoice = invoice => call(wasm.parseInvoice, invoice)

/** Parses a `contract:` contract ID. */
export const parseContractId = id => call(wasm.parseContractId, id)
