import assert from 'node:assert/strict'
import { test } from 'node:test'

import { parseContractId, v011, v012 } from '../src/index.js'

test('v0.11 contract ID round-trips and gives hex', () => {
  const id = 'rgb:eIbQx5Am-XRDjj01-RM~5eo7-rv2nluD-OnBJRAy-S9~Yfts'
  const r = parseContractId(id)
  assert.equal(r.line, 'v0.11')
  assert.equal(r.contractId, id)
  assert.match(r.hex, /^[0-9a-f]{64}$/)
})

test('v0.12 contract ID round-trips and gives hex', () => {
  const id = 'contract:nztjpjRX-_NVDDgd-EHENsqL-AKsGsyV-F6LzljB-X4WTv0Y'
  const r = parseContractId(id)
  assert.equal(r.line, 'v0.12')
  assert.equal(r.contractId, id)
  // the same ID the v0.12 invoice parser decoded natively
  assert.equal(r.hex, '9f3b63a63457f8d5430e07441c436ca8b00ab06b32545e8bce58c15f8593bf46')
})

test('contract IDs are line-specific', () => {
  assert.throws(() => v011.parseContractId('contract:nztjpjRX-_NVDDgd-EHENsqL-AKsGsyV-F6LzljB-X4WTv0Y'))
  assert.throws(() => v012.parseContractId('rgb:eIbQx5Am-XRDjj01-RM~5eo7-rv2nluD-OnBJRAy-S9~Yfts'))
})

test('bad checksum is rejected', () => {
  assert.throws(() => v011.parseContractId('rgb:eIbQx5Am-XRDjj01-RM~5eo7-rv2nluD-OnBJRAy-S9~Yftt'))
})
