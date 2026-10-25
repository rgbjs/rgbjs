// Instantiates a wasm-bindgen `--target web` module in Node or a browser.
export async function load (init, url) {
  if (typeof process !== 'undefined' && process.versions?.node) {
    const { readFile } = await import('node:fs/promises')
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
