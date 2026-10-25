# Patched copy of strict_types 2.9.1

Unchanged from crates.io except `src/value/decode.rs`: decoding lists, maps
and fixed arrays into `StrictVal` pre-allocated `Vec::with_capacity(len)` with
`len` read from the data (or from a type definition shipped with it). One
corrupted byte in a v0.12 consignment's state then makes the capacity
overflow and the decoder panics. The pinned `rgb-wallet` 0.12.0-rc.3 CLI
aborts the same way on `rgb accept`.

The patch caps the pre-allocation at 1024 items. Decoding results are
unchanged. Remove this copy once strict_types publishes a fix.

Licence: Apache-2.0, as upstream (LNP/BP Standards Association).
