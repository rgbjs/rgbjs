import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import { detectLine, parseInvoice, v011, v012 } from '../src/index.js'

const json = path => JSON.parse(readFileSync(new URL(path, import.meta.url)))

// AluVM/sonic#27: the v0.12 wallet type prints `contract:tb@contract:<id>`
// where the upstream tests (which use a narrower type) expect `contract:tb@<id>`.
// Both parse to the same invoice; compare with the doubled prefix folded.
const fold = s => s.replace(/@contract:/, '@')

for (const [lib, file] of [[v011, './vectors/v011/upstream-invoices.json'], [v012, './vectors/v012/upstream-invoices.json']]) {
  const { vectors } = json(file)
  for (const v of vectors) {
    test(`${lib.line} upstream ${v.source}: ${v.expect}`, () => {
      if (v.expect === 'ok') {
        const inv = lib.parseInvoice(v.input)
        assert.equal(inv.line, lib.line)
        if (v.roundtrip) assert.equal(lib === v012 ? fold(inv.invoice) : inv.invoice, v.input)
      } else {
        assert.throws(() => lib.parseInvoice(v.input))
      }
    })
  }
}

for (const v of json('./vectors/cli-invoices.json').vectors) {
  test(`${v.line} CLI invoice: ${v.source}`, () => {
    assert.equal(detectLine(v.input), v.line)
    const inv = parseInvoice(v.input)
    assert.equal(inv.line, v.line)
    assert.equal(inv.invoice, v.input)
    for (const [key, value] of Object.entries(v.expect)) assert.deepEqual(inv[key], value, key)
  })
}

test('each line rejects the other line\'s invoices', () => {
  const { vectors } = json('./vectors/cli-invoices.json')
  for (const v of vectors) {
    const other = v.line === 'v0.11' ? v012 : v011
    assert.throws(() => other.parseInvoice(v.input))
  }
})

test('v0.12 accepts the single and doubled contract: prefix (AluVM/sonic#27)', () => {
  const doubled = 'contract:tb@contract:nztjpjRX-_NVDDgd-EHENsqL-AKsGsyV-F6LzljB-X4WTv0Y/10@at:FWKMBVDc-NBk8dAZA-tYGt2HIP-MtzSRO17-8g3QvRIN-nuPjug/'
  const single = doubled.replace('tb@contract:', 'tb@')
  assert.equal(parseInvoice(single).contractId, parseInvoice(doubled).contractId)
})

test('detectLine', () => {
  assert.equal(detectLine('  RGB:abc'), 'v0.11')
  assert.equal(detectLine('contract:abc'), 'v0.12')
  assert.equal(detectLine('bitcoin:bc1q...'), null)
  assert.equal(detectLine(42), null)
  assert.equal(detectLine(new TextEncoder().encode('RGB\0TFR')), 'v0.11')
  assert.equal(detectLine(new TextEncoder().encode('RGBCNSGN')), 'v0.12')
  assert.equal(detectLine(new Uint8Array([1, 2, 3])), null)
})

test('parseInvoice rejects non-RGB input with a clear error', () => {
  assert.throws(() => parseInvoice('bitcoin:bc1qxyz'), /not an RGB/)
})

test('errors are Error objects with the upstream message', () => {
  assert.throws(() => v011.parseInvoice('rgb:invalid/~/~/bc:utxob:x'), err => err instanceof Error && err.message.length > 0)
})
