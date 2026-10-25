// Writes test/vectors/*/created-*.rgb from test/created-params.js, and
// test/vectors/v012/paid-fna.transfer.rgb from test/pay-params.js.
import { writeFileSync } from 'node:fs'

import { v011, v012 } from '../src/index.js'
import { SEED, v011Created, v012Created } from '../test/created-params.js'
import { toBob } from '../test/pay-params.js'

const out = p => new URL(`../test/vectors/${p}`, import.meta.url)
for (const [k, params] of Object.entries(v011Created)) writeFileSync(out(`v011/created-${k}.contract.rgb`), v011.issueContract(params))
for (const [k, params] of Object.entries(v012Created)) writeFileSync(out(`v012/created-${k}.contract.rgb`), v012.issueContract(params, SEED))
// A payment is random each run, so keep the existing sample unless asked.
if (process.argv.includes('--paid')) writeFileSync(out('v012/paid-fna.transfer.rgb'), toBob().transfer)
if (process.argv.includes('--tapret')) writeFileSync(out('v012/paid-tapret.transfer.rgb'), toBob({ commit: 'tapret' }).transfer)
console.log('written')
