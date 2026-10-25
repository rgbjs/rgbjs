import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import { assetSummary, formatAmount, validateConsignment } from '../src/index.js'

const r = p => validateConsignment(new Uint8Array(readFileSync(new URL(`./vectors/${p}`, import.meta.url))))

test('formatAmount', () => {
  assert.equal(formatAmount('10000', 5), '0.10000')
  assert.equal(formatAmount('1234567', 2), '12,345.67')
  assert.equal(formatAmount('1000000', 0), '1,000,000')
  assert.equal(formatAmount('7', 3), '0.007')
  assert.equal(formatAmount('1@2', 0), '1@2')
})

test('v0.11 NIA spec, v0.11 CFA name, v0.12 FNA globals', () => {
  const nia = assetSummary(r('v011/created-nia.contract.rgb'))
  assert.deepEqual([nia.ticker, nia.name, nia.decimals, nia.issued], ['CRT', 'Created Asset', 2, '1000000'])
  assert.equal(nia.amount('1000000'), '10,000.00 CRT')
  const cfa = assetSummary(r('v011/created-cfa.contract.rgb'))
  assert.deepEqual([cfa.ticker, cfa.name, cfa.decimals, cfa.issued], [null, 'Created Asset', 2, '5000'])
  assert.equal(cfa.amount('5000'), '50.00')
  const fna = assetSummary(r('v012/DemoToken.rgb'))
  assert.deepEqual([fna.ticker, fna.name, fna.decimals, fna.issued], ['DEMO', 'Demo Token', 5, '10000'])
  assert.equal(fna.amount('10'), '0.00010 DEMO')
  const uda = assetSummary(r('v011/created-uda.contract.rgb'))
  assert.equal(uda.amount('1@7'), '1@7')
})
