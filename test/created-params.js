// Fixed parameters and secrets for the created-contract vectors: the files in
// test/vectors/*/created-*.rgb are what these produce. Regenerate them with
// `node scripts/make-created-vectors.mjs` after an intended change.
export const OUT_A = `${'a1'.repeat(32)}:0`
export const OUT_B = `${'b2'.repeat(32)}:1`
export const SEED = new Uint8Array(32).map((_, i) => i + 1)

const common = { network: 'regtest', issuer: 'ssi:rgbjs-test', name: 'Created Asset', precision: 2, terms: 'Test asset.', timestamp: 1761400000, outpoint: OUT_A, blinding: '1234567890123' }
export const v011Created = {
  nia: { schema: 'NIA', ...common, ticker: 'CRT', supply: '1000000' },
  cfa: { schema: 'CFA', ...common, supply: '5000', details: 'A collectible.' },
  uda: { schema: 'UDA', ...common, ticker: 'CNFT', tokenIndex: 7, precision: 0 },
  ifa: { schema: 'IFA', ...common, ticker: 'CIFA', supply: '1000', maxSupply: '1500', inflationOutpoint: OUT_B, inflationBlinding: '42' },
  pfa: { schema: 'PFA', ...common, ticker: 'CPFA', supply: '777', pubkey: '02c7a3d3744b6c77f14236ece9bd8e6c25873880c8b0c709758448c8a7b9044035' }
}

const fnaIssuer = { codexId: '7C15w3W1-L0T~zXw-Aeh5~kV-Zquz729-HXQFKQW-_5lX9O8', version: 0, checksum: 'AYkSrg' }
const ufaIssuer = { codexId: 'mqmGvodN-WxpxyRb-RsGHiTQ-jDoRvMy-ClhrQGx-FMa5Cpw', version: 0, checksum: 'aiRB3w' }
const v012Base = { consensus: 'bitcoin', testnet: true, method: 'issue', timestamp: '2026-10-25T12:00:00+00:00' }
export const v012Created = {
  fna: {
    ...v012Base,
    issuer: fnaIssuer,
    name: 'CreatedToken',
    global: [{ name: 'ticker', verified: 'CRTD' }, { name: 'name', verified: 'Created Token' }, { name: 'precision', verified: 'centiMilli' }, { name: 'issued', verified: 5000 }],
    owned: [{ name: 'balance', seal: OUT_A, data: 5000 }]
  },
  ufa: {
    ...v012Base,
    issuer: ufaIssuer,
    name: 'CreatedUfa',
    global: [{ name: 'details', verified: null, unverified: 'Test asset.' }, { name: 'name', verified: 'Created UFA' }, { name: 'precision', verified: 'centi' }, { name: 'issued', verified: 900 }],
    owned: [{ name: 'balance', seal: OUT_A, data: 900 }]
  }
}
