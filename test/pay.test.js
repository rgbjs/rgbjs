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

test('payInvoice with commit: tapret has no OP_RETURN and commits in the change output', () => {
  const p = pay({ contracts: contract, invoice: bobInvoice.invoice, coins: [aliceCoin], noiseSeed: ALICE, commit: 'tapret' })
  assert.equal(p.tapret.vout, p.change.vout, 'the change is the host')
  assert.match(p.tapret.scriptPubkey, /^5120[0-9a-f]{64}$/)
  assert.ok(p.tx.includes(`22${p.tapret.scriptPubkey}`), 'the change pays to the tweaked key')
  assert.ok(!p.tx.includes('00000000000000226a20'), 'no OP_RETURN output')
  assert.notEqual(p.tapret.scriptPubkey, `5120${v012.deriveAddress(KEY, p.change.terminal).scriptPubkey.slice(4)}`, 'not the plain address')
  assert.equal(validateConsignment(p.transfer).valid, true)
  assert.deepEqual(got(p.transfer, bobCoin.outpoint, BOB, 0), ['1000'])
  assert.throws(() => pay({ contracts: contract, invoice: bobInvoice.invoice, coins: [aliceCoin], noiseSeed: ALICE, commit: 'p2c' }), /opret.*tapret/)
})

// Operations must be written in the order they happened: a later payment
// listed before the one it spends fails to validate. This chain alternates
// commit methods, so its opids sort differently from its history.
test('a chain of five payments from change validates at every step', () => {
  let files = contract
  let coin = aliceCoin
  let left = 5000
  for (const [n, commit] of ['opret', 'tapret', 'tapret', 'opret', 'tapret'].entries()) {
    const to = `${(0xd0 + n).toString(16).repeat(32)}:0`
    const inv = v012.createInvoice({ contractId, value: 100, outpoint: to, noiseSeed: CAROL, nonce: 10 + n })
    const p = pay({ contracts: files, invoice: inv.invoice, coins: [coin], noiseSeed: ALICE, commit })
    const v = validateConsignment(p.transfer)
    assert.equal(v.valid, true, `payment ${n + 1} (${commit}): ${v.error}`)
    assert.equal(v.witnesses.length, n + 1)
    assert.deepEqual(got(p.transfer, to, CAROL, 10 + n), ['100'])
    assert.equal(validateConsignment(p.backup).valid, true, `backup ${n + 1}`)
    left -= 100
    files = p.backup
    coin = { outpoint: `${p.txid}:${p.change.vout}`, sats: coin.sats - 500, terminal: p.change.terminal, tx: p.tx }
  }
  assert.equal(left, 4500)
})

// Real payments, mined on the BLAKE2b testnet4 (txbt4); see test/vectors/v012/README.md.
test('the mined txbt4 transfers validate and name their mined witnesses', async () => {
  const { readFileSync } = await import('node:fs')
  const file = n => new Uint8Array(readFileSync(new URL(`./vectors/v012/${n}`, import.meta.url)))
  const OPRET = 'c14ab82fffa1858d9721d3d94a6effaf01205c787e0732d50baba38ddc07c5d1'
  const TAPRET = 'a564889f30d6cde95c5159e9bafb436265f2bf11e3b7b6ecddf89efe5b7656c5'
  const a = validateConsignment(file('mined-opret.transfer.rgb'))
  const b = validateConsignment(file('mined-tapret.transfer.rgb'))
  assert.equal(a.valid, true, a.error)
  assert.equal(b.valid, true, b.error)
  assert.equal(a.contractId, b.contractId)
  assert.deepEqual(a.witnesses.map(w => w.txid), [OPRET])
  assert.deepEqual(b.witnesses.map(w => w.txid).sort(), [OPRET, TAPRET].sort())
})
