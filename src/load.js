// Instantiates a wasm-bindgen `--target web` module in Node or a browser.
export async function load (init, url) {
  if (typeof process !== 'undefined' && process.versions?.node) {
    // A name bundlers don't follow: this branch runs in Node only.
    const fs = 'node:fs/promises'
    const { readFile } = await import(/* @vite-ignore */ /* webpackIgnore: true */ fs)
    await init({ module_or_path: await readFile(url) })
  } else {
    await init({ module_or_path: url })
  }
}

// wasm-bindgen throws the Rust error string; turn it into an Error.
export function call (fn, ...args) {
  try {
    return JSON.parse(fn(...args))
  } catch (err) {
    if (err instanceof Error) throw err
    throw new Error(String(err))
  }
}

/**
 * Like `call`, but says what was wrong with the input: upstream messages
 * ("invalid length of identifier …") get a prefix naming the thing, unless
 * they already say it.
 */
export function explain (what, fn, ...args) {
  try {
    return call(fn, ...args)
  } catch (err) {
    if (/^(not an? |mainnet )/.test(err.message)) throw err
    throw new Error(`${what}: ${err.message}`)
  }
}
