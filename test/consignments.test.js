import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import { detectLine, v011, validateConsignment } from '../src/index.js'

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
})

test('knownSchemas lists the five rgb-schemas 0.11.1 schemas', () => {
  const names = v011.knownSchemas().map(s => s.name)
  assert.deepEqual(names, ['NIA', 'CFA', 'UDA', 'IFA', 'PFA'])
})

test('no single-byte corruption passes as the same valid contract', () => {
  // Flip one byte at a time across the body. Each result must throw, be
  // invalid, or (if the flip hit genesis data) be a different contract.
  const { contractId } = validateConsignment(contract)
  let checked = 0
  for (let i = 7; i < contract.length; i++) {
    const bad = contract.slice()
    bad[i] ^= 0x01
    let r
    try { r = validateConsignment(bad) } catch { continue }
    checked++
    assert.ok(!r.valid || r.contractId !== contractId, `byte ${i} flipped but still valid as ${contractId}`)
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

test('v0.12 consignments are not supported yet', () => {
  assert.throws(() => validateConsignment(new TextEncoder().encode('RGBCNSGN')), /not supported for v0.12/)
})
