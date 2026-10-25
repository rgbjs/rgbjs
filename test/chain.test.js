import assert from 'node:assert/strict'
import { test } from 'node:test'

import { checkWitnesses, defaultServer, ESPLORA } from '../src/index.js'

// A fake Esplora: no network in tests.
const A = 'a'.repeat(64); const B = 'b'.repeat(64); const C = 'c'.repeat(64)
function fakeFetch (calls = []) {
  return async url => {
    calls.push(url)
    const ok = (body, json = true) => ({ ok: true, status: 200, text: async () => String(body), json: async () => body })
    if (url.endsWith('/blocks/tip/height')) return ok(1000)
    if (url.endsWith(`/tx/${A}`)) return ok({ txid: A, status: { confirmed: true, block_height: 995 } })
    if (url.endsWith(`/tx/${B}`)) return ok({ txid: B, status: { confirmed: false } })
    return { ok: false, status: 404, text: async () => 'Transaction not found' }
  }
}

test('checkWitnesses: mined with confirmations, mempool, not found', async () => {
  const calls = []
  const r = await checkWitnesses([A, B, C], { server: 'https://example.test/api/', fetch: fakeFetch(calls) })
  assert.equal(r.tip, 1000)
  assert.deepEqual(r.results.map(x => [x.state, x.confirmations]), [['mined', 6], ['mempool', 0], ['not-found', 0]])
  // only the tip and the given transactions are requested
  assert.deepEqual(calls, ['https://example.test/api/blocks/tip/height', `https://example.test/api/tx/${A}`, `https://example.test/api/tx/${B}`, `https://example.test/api/tx/${C}`])
})

test('checkWitnesses refuses bad input before any request', async () => {
  const calls = []
  await assert.rejects(checkWitnesses([A], { server: 'not a url', fetch: fakeFetch(calls) }), /Esplora server URL/)
  await assert.rejects(checkWitnesses(['xyz'], { server: 'https://example.test/api', fetch: fakeFetch(calls) }), /not a transaction ID/)
  assert.deepEqual(calls, [])
})

test('checkWitnesses rejects a server that returns a different transaction', async () => {
  const lying = async url => url.endsWith('/height') ? { ok: true, text: async () => '5' } : { ok: true, json: async () => ({ txid: B, status: { confirmed: true, block_height: 1 } }) }
  await assert.rejects(checkWitnesses([A], { server: 'https://x.test', fetch: lying }), /different transaction/)
})

test('defaultServer picks the public server for the network', () => {
  assert.equal(defaultServer({ line: 'v0.11', network: 'signet' }), ESPLORA.signet)
  assert.equal(defaultServer({ line: 'v0.11', network: 'regtest' }), null)
  assert.equal(defaultServer({ line: 'v0.12', network: 'bitcoin', testnet: true }), ESPLORA.testnet4)
})

test('ESPLORA has the BLAKE2b testnet4 explorer', () => {
  assert.equal(ESPLORA.blake, 'https://mempool.guide/testnet4/api')
})
