import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import { detectLine, v011, v012, validateConsignment } from '../src/index.js'

const file = path => new Uint8Array(readFileSync(new URL(path, import.meta.url)))
const contract = file('./vectors/v011/demo.contract.rgb')

test('v0.11 contract consignment from rgb-cmd 0.11.1 is valid', () => {
  assert.equal(detectLine(contract), 'v0.11')
  const r = validateConsignment(contract)
  assert.equal(r.valid, true, r.error)
  assert.equal(r.kind, 'contract')
  assert.equal(r.contractId, 'rgb:v4Kpkuwn-yjzKgYa-5G5h0cc-lLVyr2B-hdpCTaR-KOe~agc')
  assert.equal(r.schema, 'NIA')
  assert.equal(r.schemaId, 'rgb:sch:RWhwUfTMpuP2Zfx1~j4nswCANGeJrYOqDcKelaMV4zU#remote-digital-pegasus')
  assert.equal(r.network, 'regtest')
  assert.deepEqual(r.witnesses, [])
  assert.deepEqual(r.terminals, [])
  // demo.yaml: 1000000 DEMO on the genesis coinbase output
  assert.deepEqual(r.state.global.issuedSupply, ['1000000'])
  assert.match(r.state.global.spec[0], /ticker "DEMO"/)
  assert.deepEqual(r.state.owned.assetOwner, [{
    seal: '4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b:0', value: '1000000', witness: null, spent: false
  }])
})

const transfer011 = file('./vectors/v011/transfer-demo.transfer.rgb')

test('v0.11 transfer consignment (made by rust/gen-v011) is valid', () => {
  const r = validateConsignment(transfer011)
  assert.equal(r.valid, true, r.error)
  assert.equal(r.kind, 'transfer')
  assert.equal(r.contractId, 'rgb:epeKJw9q-4ciD_0S-NChLqoR-5INEwmm-lDL1OEH-ujhEX6Y')
  assert.equal(r.schema, 'NIA')
  assert.deepEqual(r.witnesses, [{ txid: '2c54de909cbaf40fc2dedeaef1ae310442cdad45cc094f78293f63d7a94befa5', included: true }])
  // The invoice was rgb:epeKJw9q-…/~/BF/bcrt:utxob:dTwHqt40-…: Bob's blinded seal is the terminal.
  assert.deepEqual(r.terminals.flatMap(t => t.seals), ['utxob:dTwHqt40-IEPM8Yu-nFIXTvY-va2Nh7Y-xXpTu2P-F5DqjgD-0Hh4B'])
  // 100 went to Bob's blinded seal, so only the spent genesis output and the change show.
  const w = '2c54de909cbaf40fc2dedeaef1ae310442cdad45cc094f78293f63d7a94befa5'
  assert.deepEqual(r.state.owned.assetOwner, [
    { seal: '07623012f9268485f2135cbaa2c8cc2966b08b01439e27cb40be8f68147cc1c6:0', value: '1000000', witness: null, spent: true },
    { seal: `${w}:1`, value: '999900', witness: w, spent: false }
  ])
})

test('v0.11 transfer: no single-byte corruption passes as the same valid transfer', () => {
  const base = validateConsignment(transfer011)
  // A flip in the terminals leaves a valid consignment that no longer names
  // the recipient's seal; that must show in `terminals`.
  const same = JSON.stringify([base.witnesses, base.terminals, base.state])
  let checked = 0
  for (let i = 7; i < transfer011.length; i++) {
    const bad = transfer011.slice()
    bad[i] ^= 0x01
    let r
    try { r = validateConsignment(bad) } catch { continue }
    checked++
    assert.ok(!r.valid || r.contractId !== base.contractId || JSON.stringify([r.witnesses, r.terminals, r.state]) !== same,
      `byte ${i} flipped but still valid as the same transfer`)
  }
  assert.ok(checked > 0)
})

test('knownSchemas lists the five rgb-schemas 0.11.1 schemas', () => {
  const names = v011.knownSchemas().map(s => s.name)
  assert.deepEqual(names, ['NIA', 'CFA', 'UDA', 'IFA', 'PFA'])
})

test('no single-byte corruption passes as the same valid contract', () => {
  // Flip one byte at a time across the body. Each result must throw, be
  // invalid, or (if the flip hit genesis data) be a different contract.
  const { contractId, state } = validateConsignment(contract)
  const same = JSON.stringify(state)
  let checked = 0
  for (let i = 7; i < contract.length; i++) {
    const bad = contract.slice()
    bad[i] ^= 0x01
    let r
    try { r = validateConsignment(bad) } catch { continue }
    checked++
    assert.ok(!r.valid || r.contractId !== contractId || JSON.stringify(r.state) !== same,
      `byte ${i} flipped but still valid as ${contractId} with the same state`)
  }
  assert.ok(checked > 0)
})

test('truncated consignment throws', () => {
  assert.throws(() => validateConsignment(contract.subarray(0, contract.length - 10)))
})

test('0.11.2-format transfer gives a decode error, not a crash', () => {
  assert.throws(() => validateConsignment(file('./vectors/v011/nia_transfer.v0112.rgb')), /ContainerVer/)
})

test('non-consignment input throws', () => {
  assert.throws(() => validateConsignment(new Uint8Array([1, 2, 3])), /not an RGB/)
  assert.throws(() => v011.validateConsignment(new TextEncoder().encode('RGB\0KIT....')), /unsupported/)
})

const token = file('./vectors/v012/DemoToken.rgb')

test('v0.12 contract consignment from rgb-wallet 0.12.0-rc.3 is valid', () => {
  assert.equal(detectLine(token), 'v0.12')
  const r = validateConsignment(token)
  assert.equal(r.valid, true, r.error)
  assert.equal(r.contractId, 'contract:nztjpjRX-_NVDDgd-EHENsqL-AKsGsyV-F6LzljB-X4WTv0Y')
  assert.equal(r.name, 'DemoToken')
  assert.equal(r.network, 'bitcoin')
  assert.equal(r.testnet, true)
  assert.deepEqual(r.issuer, {
    id: '7C15w3W1-L0T~zXw-Aeh5~kV-Zquz729-HXQFKQW-_5lX9O8/0#AYkSrg',
    codexId: '7C15w3W1-L0T~zXw-Aeh5~kV-Zquz729-HXQFKQW-_5lX9O8#heavy-cactus-quiet',
    name: 'Fungible Non-inflatable Asset',
    developer: 'dns:pandoraprime.ch',
    version: 0,
    trusted: true
  })
  assert.equal(r.signed, false)
  assert.deepEqual(r.witnesses, [])
  // DemoToken.yaml: 10000 units of DEMO on one seal in Alice's wallet
  assert.deepEqual(r.state.global.ticker, ['"DEMO"'])
  assert.deepEqual(r.state.global.issued, ['10000'])
  assert.deepEqual(r.state.owned.balance.map(c => [c.outpoint, c.value]),
    [['b7116550736fbe5d3e234d0141c6bc8d1825f94da78514a3cede5674e9a5eae9:1', '10000']])
})

const transfer = file('./vectors/v012/TransferDemo.transfer.rgb')

test('v0.12 transfer consignment (made by rust/gen-v012) is valid', () => {
  const r = validateConsignment(transfer)
  assert.equal(r.valid, true, r.error)
  assert.equal(r.contractId, 'contract:KA73QdAH-BFBUNYX-0jsaAXq-_wKsXwD-2VLkZ9Q-1yRYYkc')
  assert.equal(r.issuer.trusted, true)
  assert.equal(r.operations, 1)
  assert.deepEqual(r.witnesses, [{ txid: 'fb0edb5f47e6c6c5f43ad8d0eb9a989da3c6cf5dd44988df76046e8a07d849fc', status: 'tentative' }])
  // Alice paid Bob 10 of 10000. Bob's seal is blinded, so only her change shows.
  assert.deepEqual(r.state.owned.balance.map(c => [c.outpoint, c.value]),
    [['fb0edb5f47e6c6c5f43ad8d0eb9a989da3c6cf5dd44988df76046e8a07d849fc:1', '9990']])
})

for (const [name, bytes] of [['DemoToken.rgb', token], ['TransferDemo.transfer.rgb', transfer]]) {
  test(`v0.12 ${name}: no single-byte corruption passes as the same trusted contract and state`, () => {
    // A flip may land in the semantics, which the contract ID doesn't commit
    // to (then the issuer must no longer be trusted), or in a revealed seal
    // definition (then that allocation must drop out of the state).
    const base = validateConsignment(bytes)
    const same = JSON.stringify([base.state, base.witnesses])
    let checked = 0
    for (let i = 8; i < bytes.length; i++) {
      const bad = bytes.slice()
      bad[i] ^= 0x01
      let r
      try { r = validateConsignment(bad) } catch { continue }
      checked++
      assert.ok(!r.valid || r.contractId !== base.contractId || !r.issuer.trusted ||
        JSON.stringify([r.state, r.witnesses]) !== same,
      `byte ${i} flipped but still valid and trusted as ${base.contractId} with the same state`)
    }
    assert.ok(checked > 0)
  })
}

test('v0.12 issuer file parses to the ID consignments report', () => {
  const issuer = v012.parseIssuer(file('../rust/v012/issuers/RGB20-FNA.issuer'))
  assert.equal(issuer.id, validateConsignment(token).issuer.id)
  assert.equal(issuer.signed, false)
  assert.deepEqual(v012.knownIssuers().map(i => i.id), [issuer.id])
  assert.throws(() => v012.parseIssuer(token), /not an RGB v0.12 issuer/)
})

test('v0.12 header checks', () => {
  assert.throws(() => v012.validateConsignment(new TextEncoder().encode('RGBCNSGN')), /not an RGB v0.12/)
  const future = token.slice()
  future[9] = 7
  assert.throws(() => v012.validateConsignment(future), /version 7/)
  assert.throws(() => v012.validateConsignment(token.subarray(0, 200)))
})
