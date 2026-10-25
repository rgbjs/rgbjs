import assert from 'node:assert/strict'
import { test } from 'node:test'

import { v012, validateConsignment } from '../src/index.js'
import { ALICE, BOB, CAROL, KEY, aliceCoin, bobCoin, bobInvoice, contract, contractId, toBob } from './pay-params.js'

const pay = o => v012.payInvoice({ descriptor: KEY, fee: 500, ...o })
const got = (file, outpoint, noiseSeed, nonce) => v012.receive(file, { outpoint, noiseSeed, nonce }).receive.received.map(x => x.value)
const C1 = `${'c3'.repeat(32)}:0`
const C2 = `${'c4'.repeat(32)}:0`

test('deriveAddress gives the testnet taproot address of the descriptor', () => {
  const a = v012.deriveAddress(KEY, '&0/0')
  assert.match(a.address, /^tb1p[0-9a-z]{58}$/)
  assert.equal(a.scriptPubkey, `5120${a.scriptPubkey.slice(4)}`)
  assert.notEqual(v012.deriveAddress(KEY, '&1/0').address, a.address)
  assert.throws(() => v012.deriveAddress('tpubnope', '&0/0'), /descriptor/)
})

test('payInvoice: Alice pays Bob, the transfer validates and Bob receives it', () => {
  const p = toBob()
  assert.match(p.txid, /^[0-9a-f]{64}$/)
  assert.match(p.psbt, /^cHNidP8/, 'a base64 PSBT')
  assert.deepEqual(p.change, { vout: 1, terminal: '&1/0' }, 'output 0 is the OP_RETURN commitment')
  const v = validateConsignment(p.transfer)
  assert.equal(v.valid, true, v.error)
  assert.equal(v.contractId, contractId)
  assert.deepEqual(v.witnesses.map(w => w.txid), [p.txid])
  assert.deepEqual(got(p.transfer, bobCoin.outpoint, BOB, 0), ['1000'])
  // The commitment tree takes fresh random entropy each time, as upstream
  // does for privacy: the same payment twice is two different transactions.
  assert.notEqual(toBob().txid, p.txid)
})

test('the paid-transfer vector validates and pays Bob', async () => {
  const { readFileSync } = await import('node:fs')
  const file = new Uint8Array(readFileSync(new URL('./vectors/v012/paid-fna.transfer.rgb', import.meta.url)))
  const v = validateConsignment(file)
  assert.equal(v.valid, true, v.error)
  assert.equal(v.contractId, contractId)
  assert.deepEqual(got(file, bobCoin.outpoint, BOB, 0), ['1000'])
})

test('payInvoice: Alice pays again from her change, using the backup it returned', () => {
  const p1 = toBob()
  const inv = v012.createInvoice({ contractId, value: 500, outpoint: C1, noiseSeed: CAROL, nonce: 1 })
  const change = { outpoint: `${p1.txid}:${p1.change.vout}`, sats: 50000 - 500, terminal: p1.change.terminal, tx: p1.tx }
  const p2 = pay({ contracts: p1.backup, invoice: inv.invoice, coins: [change], noiseSeed: ALICE })
  assert.equal(validateConsignment(p2.transfer).valid, true)
  assert.deepEqual(got(p2.transfer, C1, CAROL, 1), ['500'])
})

test('payInvoice: Bob pays onward from the transfer he received', () => {
  const inv = v012.createInvoice({ contractId, value: 300, outpoint: C2, noiseSeed: CAROL, nonce: 2 })
  const p = pay({ contracts: toBob().transfer, seals: [{ outpoint: bobCoin.outpoint, nonce: 0 }], invoice: inv.invoice, coins: [bobCoin], noiseSeed: BOB })
  assert.equal(validateConsignment(p.transfer).valid, true)
  assert.deepEqual(got(p.transfer, C2, CAROL, 2), ['300'])
  // without Bob's seed his seal can't be rebuilt, so he has nothing to spend
  assert.throws(() => pay({ contracts: toBob().transfer, seals: [{ outpoint: bobCoin.outpoint, nonce: 0 }], invoice: inv.invoice, coins: [bobCoin], noiseSeed: CAROL }), /not enough/)
})

test('payInvoice refuses overspending and inconsistent input', () => {
  const big = v012.createInvoice({ contractId, value: 6000, outpoint: C2, noiseSeed: CAROL, nonce: 3 })
  assert.throws(() => pay({ contracts: contract, invoice: big.invoice, coins: [aliceCoin], noiseSeed: ALICE }), /not enough/)
  assert.throws(() => pay({ contracts: contract, invoice: bobInvoice.invoice, coins: [{ ...aliceCoin, tx: bobCoin.tx }], noiseSeed: ALICE }), /has txid/)
  assert.throws(() => pay({ contracts: contract, invoice: bobInvoice.invoice, coins: [{ ...aliceCoin, sats: 1 }], noiseSeed: ALICE }), /holds 50000 sats/)
  assert.throws(() => pay({ contracts: contract, invoice: bobInvoice.invoice, coins: [aliceCoin], noiseSeed: ALICE.slice(0, 16) }), /32 bytes/)
  assert.throws(() => pay({ contracts: new Uint8Array(20), invoice: bobInvoice.invoice, coins: [aliceCoin], noiseSeed: ALICE }), /RGBCNSGN/)
})
