// Optional on-chain check of witness transactions against an Esplora API.
// This is the only rgbjs code that uses the network, and only when called:
// the server learns which transaction IDs you ask about, nothing else.

/** Public Esplora APIs by network. Any other Esplora URL works too. */
export const ESPLORA = {
  bitcoin: 'https://mempool.space/api',
  testnet4: 'https://mempool.space/testnet4/api',
  testnet3: 'https://mempool.space/testnet/api',
  signet: 'https://mempool.space/signet/api'
}

/**
 * Looks up each witness transaction on `server` (an Esplora base URL).
 * Resolves to { server, tip, results: [{ txid, state, height, confirmations }] }
 * where state is 'mined', 'mempool' or 'not-found'. A transaction the server
 * doesn't know may not be broadcast yet, or be on another network.
 */
export async function checkWitnesses (txids, { server, fetch = globalThis.fetch } = {}) {
  if (!server || !/^https?:\/\/[^\s]+$/.test(server)) throw new Error('Give an Esplora server URL, such as https://mempool.space/signet/api')
  const base = server.replace(/\/+$/, '')
  for (const txid of txids) if (!/^[0-9a-f]{64}$/.test(txid)) throw new Error(`not a transaction ID: ${txid}`)
  const tipRes = await fetch(`${base}/blocks/tip/height`)
  if (!tipRes.ok) throw new Error(`${base} answered ${tipRes.status} for the chain tip; is it an Esplora API?`)
  const tip = Number(await tipRes.text())
  if (!Number.isInteger(tip)) throw new Error(`${base} didn't return a block height`)
  const results = []
  for (const txid of txids) {
    const res = await fetch(`${base}/tx/${txid}`)
    if (!res.ok) { results.push({ txid, state: 'not-found', height: null, confirmations: 0 }); continue }
    const tx = await res.json()
    if (tx?.txid !== txid) throw new Error(`${base} returned a different transaction for ${txid}`)
    const { confirmed, block_height: height } = tx.status ?? {}
    results.push(confirmed
      ? { txid, state: 'mined', height, confirmations: Math.max(0, tip - height + 1) }
      : { txid, state: 'mempool', height: null, confirmations: 0 })
  }
  return { server: base, tip, results }
}

/** The public server for a result's network, if there is one (none for regtest). */
export function defaultServer (result) {
  if (result?.line === 'v0.11') return ESPLORA[result.network] ?? null
  // v0.12 results say only bitcoin + testnet; rgb-wallet 0.12.0-rc.3 defaults to testnet4.
  if (result?.line === 'v0.12') return result.network === 'bitcoin' ? (result.testnet ? ESPLORA.testnet4 : ESPLORA.bitcoin) : null
  return null
}
