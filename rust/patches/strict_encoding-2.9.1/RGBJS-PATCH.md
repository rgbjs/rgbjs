# Patched copy of strict_encoding 2.9.1

Unchanged from crates.io except `src/embedded.rs`: `Confined<Vec<T>>` and
`Confined<VecDeque<T>>` decoding pre-allocated `Vec::with_capacity(len)` with
`len` taken straight from the input. A consignment with one corrupted length
byte (for example a witness transaction's input count) then asks for ~4.4 GB:
in wasm that aborts with `unreachable` instead of returning a decode error.

The patch caps the pre-allocation at a 64 KiB budget, as rgb-protocol's
`rgb-strict-encoding` 1.0.5 does (`prealloc_cap`). Decoding results are
unchanged. Remove this copy once strict_encoding publishes a fix.

Licence: Apache-2.0, as upstream (LNP/BP Standards Association).
