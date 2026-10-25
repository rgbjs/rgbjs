import assert from 'node:assert/strict'
import { test } from 'node:test'

import { detectLine, parseInvoice, v011, v012, validateConsignment } from '../src/index.js'

// Made-up outpoints: creation is offline, and the seals only need to exist on
// chain once the asset is used.
const OUT_A = `${'a1'.repeat(32)}:0`
const OUT_B = `${'b2'.repeat(32)}:1`
const SEED = new Uint8Array(32).map((_, i) => i + 1)

// Every corruption of a created file must be rejected or visibly different.
function assertTamperEvident (bytes) {
  const base = validateConsignment(bytes)
  const fp = r => JSON.stringify([r.contractId, r.issuer?.trusted, r.terminals, r.witnesses, r.state])
  for (let i = 8; i < bytes.length; i += 7) {
    const bad = bytes.slice()
    bad[i] ^= 0x01
    let r
    try { r = validateConsignment(bad) } catch (e) { assert.doesNotMatch(e.message, /unreachable|RuntimeError/); continue }
    assert.ok(!r.valid || fp(r) !== fp(base), `byte ${i} flipped but still the same valid contract`)
  }
}

const v011Common = { network: 'regtest', issuer: 'ssi:rgbjs-test', name: 'Created Asset', precision: 2, terms: 'Test asset.', timestamp: 1761400000, outpoint: OUT_A, blinding: '1234567890123' }
const v011Cases = {
  NIA: { ticker: 'CRT', supply: '1000000' },
  CFA: { supply: '5000', details: 'A collectible.' },
  UDA: { ticker: 'CNFT', tokenIndex: 7, precision: 0 },
  IFA: { ticker: 'CIFA', supply: '1000', maxSupply: '1500', inflationOutpoint: OUT_B, inflationBlinding: '42' },
  PFA: { ticker: 'CPFA', supply: '777', pubkey: '02c7a3d3744b6c77f14236ece9bd8e6c25873880c8b0c709758448c8a7b9044035' }
}

for (const [schema, extra] of Object.entries(v011Cases)) {
  test(`v0.11 issue ${schema}: valid, with the given state, and tamper-evident`, () => {
    const bytes = v011.issueContract({ schema, ...v011Common, ...extra })
    assert.equal(detectLine(bytes), 'v0.11')
    const r = validateConsignment(bytes)
    assert.equal(r.valid, true, r.error)
    assert.equal(r.schema, schema)
    assert.equal(r.network, 'regtest')
    assert.match(r.contractId, /^rgb:/)
    const cells = Object.values(r.state.owned).flat()
    assert.ok(cells.some(c => c.seal === OUT_A), 'the asset sits on the given outpoint')
    if (schema === 'UDA') assert.deepEqual(r.state.owned.assetOwner.map(c => c.value), ['1@7'])
    else assert.deepEqual(r.state.global.issuedSupply, [extra.supply])
    if (schema === 'IFA') assert.deepEqual(r.state.owned.inflationAllowance.map(c => [c.seal, c.value]), [[OUT_B, '500']])
    // Same input, same contract: no hidden randomness.
    assert.deepEqual(v011.issueContract({ schema, ...v011Common, ...extra }), bytes)
    assertTamperEvident(bytes)
  })
}

test('v0.11 issue refuses mainnet and bad input', () => {
  assert.throws(() => v011.issueContract({ schema: 'NIA', ...v011Common, ...v011Cases.NIA, network: 'bitcoin' }), /mainnet is not supported/)
  assert.throws(() => v011.issueContract({ schema: 'XYZ', ...v011Common }), /unknown schema/)
  assert.throws(() => v011.issueContract({ schema: 'NIA', ...v011Common, ...v011Cases.NIA, precision: 19 }), /precision/)
  assert.throws(() => v011.issueContract({ schema: 'IFA', ...v011Common, ...v011Cases.IFA, maxSupply: '10' }), /maxSupply/)
  assert.throws(() => v011.issueContract({ schema: 'NIA', ...v011Common, ...v011Cases.NIA, outpoint: 'nope' }), /txid:vout/)
})

test('v0.11 invoice: parses back with the contract, amount and blinded seal', () => {
  const contractId = validateConsignment(v011.issueContract({ schema: 'NIA', ...v011Common, ...v011Cases.NIA })).contractId
  const created = v011.createInvoice({ network: 'regtest', outpoint: OUT_B, blinding: '987654321', contractId, amount: '250', assignmentName: 'assetOwner', endpoints: ['rpcs://proxy.example.com/json-rpc'] })
  const inv = parseInvoice(created.invoice)
  assert.equal(inv.line, 'v0.11')
  assert.equal(inv.contractId, contractId)
  assert.equal(inv.amount, '250')
  assert.equal(inv.network, 'regtest')
  assert.equal(inv.beneficiary.value, `bcrt:${created.secretSeal}`)
  assert.deepEqual(created.seal, { outpoint: OUT_B, blinding: '987654321' })
  assert.deepEqual(inv.transports, [{ type: 'json-rpc', tls: true, host: 'proxy.example.com/json-rpc' }])
  // the same seal always blinds the same way; a different blinding doesn't
  assert.equal(v011.createInvoice({ network: 'regtest', outpoint: OUT_B, blinding: '987654321' }).secretSeal, created.secretSeal)
  assert.notEqual(v011.createInvoice({ network: 'regtest', outpoint: OUT_B, blinding: '987654322' }).secretSeal, created.secretSeal)
  assert.throws(() => v011.createInvoice({ network: 'bitcoin', outpoint: OUT_B, blinding: '1' }), /mainnet/)
})

// The DemoToken parameters from RGB-WG/rgb v0.12.0-rc.3 examples, as JSON.
const fna = {
  consensus: 'bitcoin',
  testnet: true,
  issuer: { codexId: '7C15w3W1-L0T~zXw-Aeh5~kV-Zquz729-HXQFKQW-_5lX9O8', version: 0, checksum: 'AYkSrg' },
  name: 'CreatedToken',
  method: 'issue',
  timestamp: '2026-10-25T12:00:00+00:00',
  global: [
    { name: 'ticker', verified: 'CRTD' },
    { name: 'name', verified: 'Created Token' },
    { name: 'precision', verified: 'centiMilli' },
    { name: 'issued', verified: 5000 }
  ],
  owned: [{ name: 'balance', seal: OUT_A, data: 5000 }]
}

test('v0.12 issue (RGB20 FNA): valid, trusted, with the given state, and tamper-evident', () => {
  const bytes = v012.issueContract(fna, SEED)
  assert.equal(detectLine(bytes), 'v0.12')
  const r = validateConsignment(bytes)
  assert.equal(r.valid, true, r.error)
  assert.equal(r.name, 'CreatedToken')
  assert.equal(r.issuer.trusted, true)
  assert.deepEqual(r.state.global.ticker, ['"CRTD"'])
  assert.deepEqual(r.state.owned.balance.map(c => [c.outpoint, c.value]), [[OUT_A, '5000']])
  assert.deepEqual(v012.issueContract(fna, SEED), bytes, 'deterministic for the same seed')
  assert.notDeepEqual(v012.issueContract(fna, SEED.map(x => x ^ 0xff)), bytes, 'the seed blinds the seals')
  assertTamperEvident(bytes)
})

test('v0.12 issue refuses mainnet, short seeds and unknown issuers', () => {
  assert.throws(() => v012.issueContract({ ...fna, testnet: false }, SEED), /mainnet/)
  assert.throws(() => v012.issueContract(fna, new Uint8Array(4)), /noise seed/)
  assert.throws(() => v012.issueContract({ ...fna, issuer: { ...fna.issuer, checksum: 'AAAAAA' } }, SEED), /no known issuer|checksum|issuer/)
})

test('v0.12 invoice: parses back with the contract, value and auth token', () => {
  const contractId = validateConsignment(v012.issueContract(fna, SEED)).contractId
  const created = v012.createInvoice({ contractId, value: 25, outpoint: OUT_B, noiseSeed: SEED, nonce: 3 })
  const inv = parseInvoice(created.invoice)
  assert.equal(inv.line, 'v0.12')
  assert.equal(inv.contractId, contractId)
  assert.equal(inv.amount, '25')
  assert.deepEqual(inv.beneficiary, { type: 'auth-token', value: created.authToken })
  assert.equal(created.invoice, `contract:tb@${contractId.slice('contract:'.length)}/25@${created.authToken}/`)
  // nonce and seed both change the token
  assert.notEqual(v012.createInvoice({ contractId, value: 25, outpoint: OUT_B, noiseSeed: SEED, nonce: 4 }).authToken, created.authToken)
  assert.throws(() => v012.createInvoice({ contractId, value: 'a/b', outpoint: OUT_B, noiseSeed: SEED }), /plain string/)
})

test('created-contract vectors are reproduced byte for byte', async () => {
  const { readFileSync } = await import('node:fs')
  const { SEED, v011Created, v012Created } = await import('./created-params.js')
  const file = p => new Uint8Array(readFileSync(new URL(`./vectors/${p}`, import.meta.url)))
  for (const [k, p] of Object.entries(v011Created)) assert.deepEqual(v011.issueContract(p), file(`v011/created-${k}.contract.rgb`), k)
  for (const [k, p] of Object.entries(v012Created)) {
    const bytes = v012.issueContract(p, SEED)
    assert.deepEqual(bytes, file(`v012/created-${k}.contract.rgb`), k)
    const r = validateConsignment(bytes)
    assert.equal(r.valid, true, r.error)
    assert.equal(r.issuer.trusted, true, k)
  }
})

test('v0.12 invoice with an expiry: written in RFC 3339 UTC and read back', () => {
  const contractId = validateConsignment(v012.issueContract(fna, SEED)).contractId
  const at = 1793016000 // 2026-10-26T12:00:00Z
  const created = v012.createInvoice({ contractId, value: 25, outpoint: OUT_B, noiseSeed: SEED, nonce: 3, expiry: at })
  assert.match(created.invoice, /\/\?expiry=2026-10-26T12:00:00Z$/)
  assert.equal(parseInvoice(created.invoice).expiry, at)
  assert.equal(parseInvoice(v012.createInvoice({ contractId, value: 25, outpoint: OUT_B, noiseSeed: SEED, nonce: 3 }).invoice).expiry, null)
})
