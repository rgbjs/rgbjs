// The README documents what the code does: every export is in its API table,
// and its example values come out of real calls.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import * as rgbjs from '../src/index.js'
import * as v011 from '../src/v011.js'
import * as v012 from '../src/v012.js'

const readme = readFileSync(new URL('../README.md', import.meta.url), 'utf8')
const table = readme.slice(readme.indexOf('### API'))

test('every export is in the README\'s API table', () => {
  for (const [from, mod] of [['rgbjs', rgbjs], ['v011', v011], ['v012', v012]]) {
    for (const name of Object.keys(mod)) assert.ok(table.includes(`\`${name}\``), `${from}.${name} is missing from the API table`)
  }
})

test('the README\'s invoice example gives what it says', () => {
  const inv = rgbjs.parseInvoice('rgb:Tk3d0h5w-8v4XYCg-7e~Sc0o-Lu6rp~X-~Jt7VHS-jqgzFD8/~/BF/bcrt:utxob:MSLQCKkW-w6caphB-12do1nJ-HNfgxvv-WE3zOOC-s8yZyHq-Ihoa5')
  assert.equal(rgbjs.detectLine('rgb:Tk3d0h5w-'), 'v0.11')
  assert.equal(rgbjs.detectLine('contract:tb@x'), 'v0.12')
  assert.equal(inv.line, 'v0.11')
  assert.equal(inv.network, 'regtest')
  assert.equal(inv.contractId, 'rgb:Tk3d0h5w-8v4XYCg-7e~Sc0o-Lu6rp~X-~Jt7VHS-jqgzFD8')
  assert.equal(inv.amount, '100')
  assert.equal(inv.beneficiary.type, 'blinded-utxo')
})

test('payInvoice returns the fields the README lists', async () => {
  const { ALICE, KEY, aliceCoin, bobInvoice, contract } = await import('./pay-params.js')
  for (const commit of ['opret', 'tapret']) {
    const p = v012.payInvoice({ contracts: [contract], invoice: bobInvoice.invoice, descriptor: KEY, coins: [aliceCoin], fee: 500, noiseSeed: ALICE, commit })
    assert.deepEqual(Object.keys(p).sort(), ['backup', 'change', 'fee', 'line', 'psbt', 'tapret', 'transfer', 'tx', 'txid'].sort())
    assert.ok(p.transfer instanceof Uint8Array && p.backup instanceof Uint8Array)
    assert.match(p.tx, /^[0-9a-f]+$/)
    assert.match(p.psbt, /^cHNidP8/)
    assert.deepEqual(Object.keys(p.change).sort(), ['terminal', 'vout'])
    if (commit === 'tapret') assert.deepEqual(Object.keys(p.tapret).sort(), ['merkleRoot', 'scriptPubkey', 'vout'])
    else assert.equal(p.tapret, null)
  }
})

test('ESPLORA has the networks the README lists', () => {
  assert.deepEqual(Object.keys(rgbjs.ESPLORA).sort(), ['bitcoin', 'blake', 'signet', 'testnet3', 'testnet4'])
})
