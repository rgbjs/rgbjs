// A fixed payment for the paid-transfer vector and the pay tests: Alice
// issues on a made-up coin her descriptor owns, then pays Bob 1000 to his
// own made-up coin. test/vectors/v012/paid-fna.transfer.rgb is one run of
// `toBob()`: payments use fresh random commitment entropy, so a rerun of
// `node scripts/make-created-vectors.mjs` writes a different, equally valid file.
import { createHash } from 'node:crypto'

import { v012, validateConsignment } from '../src/index.js'

// A public BIP84 test key from RGB-WG/rgb v0.12.0-rc.3 examples/data (Alice).
export const KEY = '[cf05f8ea/84h/1h/0h]tpubDCpZeo9QAvMetovHqqqMgzRnWU2Wy5cgZUG9g3v6SJLihmARTw2sSNga8xfZTqqAFJpoTgEnVjwLjUEmf36GZ2gzba5w3GbjDbxLitzDGZA/<0;1>/*'
export const [ALICE, BOB, CAROL] = [7, 9, 11].map(x => new Uint8Array(32).fill(x))

const hex = b => Buffer.from(b).toString('hex')

/** A made-up, never-broadcast transaction paying `sats` to KEY at `terminal`. */
export function coin (terminal, sats, salt) {
  const { scriptPubkey } = v012.deriveAddress(KEY, terminal)
  const value = Buffer.alloc(8)
  value.writeBigUInt64LE(BigInt(sats))
  const tx = '02000000' + '01' + salt.repeat(32) + '00000000' + '00' + 'fdffffff' + '01' + hex(value) + hex([scriptPubkey.length / 2]) + scriptPubkey + '00000000'
  const id = createHash('sha256').update(createHash('sha256').update(Buffer.from(tx, 'hex')).digest()).digest()
  return { outpoint: `${hex(Buffer.from(id).reverse())}:0`, sats, terminal, tx }
}

export const aliceCoin = coin('&0/0', 50000, '11')
export const bobCoin = coin('&0/5', 20000, '22')

export const contract = v012.issueContract({
  consensus: 'bitcoin',
  testnet: true,
  method: 'issue',
  timestamp: '2026-10-25T12:00:00+00:00',
  name: 'PayDemo',
  issuer: { codexId: '7C15w3W1-L0T~zXw-Aeh5~kV-Zquz729-HXQFKQW-_5lX9O8', version: 0, checksum: 'AYkSrg' },
  global: [{ name: 'ticker', verified: 'PAY' }, { name: 'name', verified: 'Pay Demo' }, { name: 'precision', verified: 'centiMilli' }, { name: 'issued', verified: 5000 }],
  owned: [{ name: 'balance', seal: aliceCoin.outpoint, data: 5000 }]
}, ALICE)
export const contractId = validateConsignment(contract).contractId

export const bobInvoice = v012.createInvoice({ contractId, value: 1000, outpoint: bobCoin.outpoint, noiseSeed: BOB, nonce: 0 })

export const toBob = (o = {}) => v012.payInvoice({ contracts: contract, invoice: bobInvoice.invoice, descriptor: KEY, coins: [aliceCoin], fee: 500, noiseSeed: ALICE, ...o })
