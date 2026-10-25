import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import { v011, v012, validateConsignment } from '../src/index.js'

const file = path => new Uint8Array(readFileSync(new URL(path, import.meta.url)))

// One transfer per further v0.11 schema, from rust/gen-v011 (see vectors/v011/README.md).
const v011Cases = [
  ['cfa.transfer.rgb', 'CFA', 'rgb:58i4t437-OMIDqRp-zZE~8GJ-5UMiOjm-8BZv0Ph-qq2uDMY', s => {
    assert.deepEqual(s.global.issuedSupply, ['100000'])
    assert.deepEqual(s.owned.assetOwner.filter(c => !c.spent).map(c => c.value), ['99900'])
  }],
  ['uda.transfer.rgb', 'UDA', 'rgb:Do79LBkm-YvaKZJ4-I7~LRmq-WZtdzuL-0scOJpk-uIeUz_E', s => {
    // the whole token (fraction 1 of index 2) went to Bob's blinded seal
    assert.deepEqual(s.owned.assetOwner.map(c => [c.value, c.spent]), [['1@2', true]])
  }],
  ['pfa.transfer.rgb', 'PFA', 'rgb:cw2EyZvk-VojyG1R-_D17TW2-SSzy116-9jaCcot-rO3uMrU', s => {
    assert.match(s.global.pubkey[0], /^0x0[23][0-9a-f]{64}$/)
    assert.deepEqual(s.owned.assetOwner.filter(c => !c.spent).map(c => c.value), ['99900'])
  }],
  ['ifa.transfer.rgb', 'IFA', 'rgb:csj9hHYN-eeOsHBR-HNst0lt-3ww6bzI-THWd_5H-yr743tQ', s => {
    assert.deepEqual(s.global.maxSupply, ['150000'])
    assert.deepEqual(s.owned.inflationAllowance.map(c => [c.value, c.spent]), [['50000', false]])
    assert.deepEqual(s.owned.assetOwner.filter(c => !c.spent).map(c => c.value), ['99900'])
  }]
]

for (const [name, schema, id, checkState] of v011Cases) {
  test(`v0.11 ${schema} transfer is valid`, () => {
    const r = v011.validateConsignment(file(`./vectors/v011/${name}`))
    assert.equal(r.valid, true, r.error)
    assert.equal(r.schema, schema)
    assert.equal(r.contractId, id)
    assert.equal(r.terminals.flatMap(t => t.seals).length, 1)
    checkState(r.state)
  })
}

test('v0.11 PFA transfer without the issuer signature is rejected', () => {
  // The same transfer as pfa.transfer.rgb before rust/gen-v011 signs it: the
  // PFA script's `vts` finds no signature and fails.
  const r = v011.validateConsignment(file('./vectors/v011/pfa-unsigned.invalid.rgb'))
  assert.equal(r.valid, false)
  assert.match(r.error, /evaluation of AluVM script .* failed with the code Some\(21\)/)
})

test('v0.12 RGB25 transfer (pandora-prime issuer) is valid and trusted', () => {
  const r = v012.validateConsignment(file('./vectors/v012/UfDemo.transfer.rgb'))
  assert.equal(r.valid, true, r.error)
  assert.equal(r.contractId, 'contract:5rJSmyxc-Pm0qywU-NQWQLSZ-BqHTJ03-NjotSn5-3UzzihA')
  assert.equal(r.issuer.name, 'Uniquely Fungible Asset')
  assert.equal(r.issuer.trusted, true)
  assert.equal(r.operations, 1)
  assert.deepEqual(r.state.owned.balance.map(c => c.value), ['9990'])
})

test('v0.12 RGB21 NFT contract is valid and trusted, with unverified token data apart', () => {
  const r = v012.validateConsignment(file('./vectors/v012/NftDemo.contract.rgb'))
  assert.equal(r.valid, true, r.error)
  assert.equal(r.contractId, 'contract:7da2xRjy-WPK13jt-XWAiw6A-TggqbKK-xyNAvKd-UI0V~7E')
  assert.equal(r.issuer.name, 'Non-Fungible Unique Asset')
  assert.equal(r.issuer.trusted, true)
  assert.deepEqual(r.state.owned.balance.map(c => c.value), ['tokenNo 1, fractions 1'])
  assert.deepEqual(r.state.global.token, ['1'])
  assert.match(r.state.unverified.token[0], /Demo token one/)
})

test('knownIssuers lists RGB20 FNA, RGB21 UniqueNFT and RGB25 UniquelyFungible', () => {
  assert.deepEqual(v012.knownIssuers().map(i => i.name).sort(),
    ['Fungible Non-inflatable Asset', 'Non-Fungible Unique Asset', 'Uniquely Fungible Asset'])
})

// A sample (every 5th byte) of single-byte corruptions of each new file; the
// full sweep runs in rgb-conformance's tools/make-vectors.mjs.
for (const path of ['v011/pfa.transfer.rgb', 'v011/cfa.transfer.rgb', 'v011/uda.transfer.rgb', 'v011/ifa.transfer.rgb', 'v012/UfDemo.transfer.rgb', 'v012/NftDemo.contract.rgb']) {
  test(`${path}: sampled corruptions are rejected or visibly different`, () => {
    const bytes = file(`./vectors/${path}`)
    const base = validateConsignment(bytes)
    const fp = r => JSON.stringify([r.contractId, r.issuer?.trusted, r.terminals, r.witnesses, r.state])
    for (let i = 8; i < bytes.length; i += 5) {
      const bad = bytes.slice()
      bad[i] ^= 0x01
      let r
      try { r = validateConsignment(bad) } catch (e) { assert.doesNotMatch(e.message, /unreachable|RuntimeError/, `byte ${i}: wasm trap`); continue }
      assert.ok(!r.valid || fp(r) !== fp(base), `byte ${i} flipped but still valid with the same result`)
    }
  })
}
