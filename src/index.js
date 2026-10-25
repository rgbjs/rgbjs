// rgbjs: one API over both RGB lines. See README.md.
import * as v011 from './v011.js'
import * as v012 from './v012.js'

export { v011, v012 }
export { assetSummary, formatAmount } from './summary.js'
export { checkWitnesses, defaultServer, ESPLORA } from './chain.js'

const V011_FILE = 'RGB\0'
const V012_FILE = 'RGBCNSGN'

/**
 * Says which RGB line a string (invoice or contract ID) or a binary file
 * belongs to: 'v0.11', 'v0.12', or null if it is neither.
 */
export function detectLine (input) {
  if (typeof input === 'string') {
    const s = input.trim().toLowerCase()
    if (s.startsWith('rgb:')) return 'v0.11'
    if (s.startsWith('contract:')) return 'v0.12'
    return null
  }
  if (input instanceof Uint8Array) {
    const head = String.fromCharCode(...input.subarray(0, 8))
    if (head.startsWith(V011_FILE)) return 'v0.11'
    if (head === V012_FILE) return 'v0.12'
    return null
  }
  return null
}

function pick (input) {
  const line = detectLine(input)
  if (line === 'v0.11') return v011
  if (line === 'v0.12') return v012
  throw new Error(input instanceof Uint8Array
    ? 'not an RGB v0.11 (RGB\\0…) or v0.12 (RGBCNSGN) file'
    : 'not an RGB v0.11 (rgb:) or v0.12 (contract:) value')
}

/** Parses an invoice from either line. The result's `line` says which. */
export const parseInvoice = invoice => pick(invoice).parseInvoice(invoice.trim())

/** Parses a contract ID from either line. */
export const parseContractId = id => pick(id).parseContractId(id.trim())

/** Validates a consignment file from either line offline. */
export const validateConsignment = bytes => pick(bytes).validateConsignment(bytes)
