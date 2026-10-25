import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import { v011, v012 } from '../src/index.js'

const file = p => new Uint8Array(readFileSync(new URL(`./vectors/${p}`, import.meta.url)))
const json = p => JSON.parse(readFileSync(new URL(`./vectors/${p}`, import.meta.url)))
const hexBytes = h => Uint8Array.from(h.match(/../g), b => parseInt(b, 16))

// The full loop: rgbjs made the invoice, the reference wallet library paid
// it (rust/gen-v011 and gen-v012), and rgbjs finds the payment with the secret.
test('v0.11: the invoice secret finds what a transfer paid', () => {
  const s = json('v011/receive-nia.secret.json')
  const r = v011.receive(file('v011/receive-nia.transfer.rgb'), s)
  assert.equal(r.valid, true, r.error)
  assert.equal(r.receive.addressedToYou, true)
  assert.equal(r.receive.secretSeal, s.invoice.match(/utxob:[^?]+/)[0])
  assert.deepEqual(r.receive.received.map(x => [x.name, x.value, x.outpoint]), [['assetOwner', '100', s.outpoint]])
  assert.match(r.receive.received[0].witness, /^[0-9a-f]{64}$/)
})

test('v0.11: a wrong secret or another transfer finds nothing', () => {
  const s = json('v011/receive-nia.secret.json')
  const wrong = v011.receive(file('v011/receive-nia.transfer.rgb'), { ...s, blinding: '424243' })
  assert.equal(wrong.receive.addressedToYou, false)
  assert.deepEqual(wrong.receive.received, [])
  const other = v011.receive(file('v011/transfer-demo.transfer.rgb'), s)
  assert.equal(other.valid, true)
  assert.equal(other.receive.addressedToYou, false)
})

test('v0.12: the invoice secret finds what a transfer paid', () => {
  const s = json('v012/receive-fna.secret.json')
  const r = v012.receive(file('v012/receive-fna.transfer.rgb'), { ...s, noiseSeed: hexBytes(s.noiseSeed) })
  assert.equal(r.valid, true, r.error)
  assert.equal(r.receive.authToken, s.authToken)
  assert.equal(r.receive.addressedToYou, true)
  assert.deepEqual(r.receive.received.map(x => [x.name, x.value, x.outpoint]), [['balance', '10', s.outpoint]])
  assert.match(r.receive.received[0].witness, /^[0-9a-f]{64}$/)
})

test('v0.12: a wrong seed or nonce finds nothing', () => {
  const s = json('v012/receive-fna.secret.json')
  for (const bad of [{ noiseSeed: hexBytes(s.noiseSeed.replace(/^5e/, '5f')) }, { noiseSeed: hexBytes(s.noiseSeed), nonce: 1 }]) {
    const r = v012.receive(file('v012/receive-fna.transfer.rgb'), { ...s, ...bad })
    assert.equal(r.receive.addressedToYou, false)
  }
})

// No corruption can change how much you appear to receive, or where: a
// changed byte is rejected, finds nothing, makes the v0.12 issuer untrusted
// (a change in the semantics, which the contract ID doesn't cover), or (in
// an unsigned witness's uncommitted fields) changes only the witness ID.
for (const [line, path, secret] of [
  ['v011', 'v011/receive-nia.transfer.rgb', () => json('v011/receive-nia.secret.json')],
  ['v012', 'v012/receive-fna.transfer.rgb', () => { const s = json('v012/receive-fna.secret.json'); return { ...s, noiseSeed: hexBytes(s.noiseSeed) } }]
]) {
  test(`${line}: no corruption changes what receive reports`, () => {
    const lib = line === 'v011' ? v011 : v012
    const bytes = file(path)
    const s = secret()
    const base = lib.receive(bytes, s).receive.received.map(x => [x.name, x.value, x.outpoint])
    for (let i = 8; i < bytes.length; i += 3) {
      const bad = bytes.slice()
      bad[i] ^= 0x01
      let r
      try { r = lib.receive(bad, s) } catch (e) { assert.doesNotMatch(e.message, /unreachable|RuntimeError/); continue }
      if (!r.valid || !r.receive.received.length || (r.issuer && !r.issuer.trusted)) continue
      assert.deepEqual(r.receive.received.map(x => [x.name, x.value, x.outpoint]), base, `byte ${i}`)
    }
  })
}
