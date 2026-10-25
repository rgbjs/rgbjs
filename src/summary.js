// Turns a validation result's raw state into a readable asset summary.
// Pure JavaScript: works on results from either line.

const PRECISION = ['indivisible', 'deci', 'centi', 'milli', 'deciMilli', 'centiMilli', 'micro', 'deciMicro',
  'centiMicro', 'nano', 'deciNano', 'centiNano', 'pico', 'deciPico', 'centiPico', 'femto', 'deciFemto',
  'centiFemto', 'atto']

const unquote = s => (s ?? '').replace(/^"(.*)"$/, '$1')
const first = (g, k) => (g?.[k] ?? [])[0]

/** Formats an integer amount in the smallest unit with `decimals` places. */
export function formatAmount (raw, decimals = 0) {
  if (!/^\d+$/.test(String(raw ?? ''))) return String(raw ?? '')
  const s = String(raw).padStart(decimals + 1, '0')
  const whole = s.slice(0, s.length - decimals).replace(/\B(?=(\d{3})+(?!\d))/g, ',')
  return decimals ? `${whole}.${s.slice(-decimals)}` : whole
}

/**
 * Reads ticker, name, decimal places and issued supply from a
 * validateConsignment() result. Missing fields are null. `amount(raw)`
 * formats a raw amount of this asset, e.g. '10000' → '0.10000 DEMO'.
 */
export function assetSummary (result) {
  const g = result?.state?.global ?? {}
  let ticker = null
  let name = null
  let precision = null
  const spec = first(g, 'spec') // v0.11: 'ticker "DEMO", name "Demo Asset", details ~, precision centiMilli'
  if (spec) {
    ticker = spec.match(/ticker "([^"]*)"/)?.[1] ?? null
    name = spec.match(/name "([^"]*)"/)?.[1] ?? null
    precision = spec.match(/precision (\w+)/)?.[1] ?? null
  }
  ticker ??= first(g, 'ticker') ? unquote(first(g, 'ticker')) : null
  name ??= first(g, 'name') ? unquote(first(g, 'name')) : null
  precision ??= first(g, 'precision') ?? null
  const decimals = precision == null ? 0 : Math.max(0, PRECISION.indexOf(precision))
  const issued = first(g, 'issuedSupply') ?? first(g, 'issued') ?? null
  const unit = ticker ?? ''
  const amount = raw => /^\d+$/.test(String(raw ?? '')) ? `${formatAmount(raw, decimals)}${unit ? ' ' + unit : ''}` : String(raw ?? '')
  return { ticker, name, decimals, issued, amount }
}
