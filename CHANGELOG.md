# Changelog

## 0.0.2 (unreleased)

The first release with working code. Both RGB lines, each built from its own Rust crates compiled to WebAssembly.

- **Both lines:** parse invoices and contract IDs, tell the lines apart, validate contracts and transfers offline, issue contracts and make invoices on test networks, and see what a transfer pays you.
- **v0.12:** pay an invoice (`payInvoice`), returning an unsigned PSBT, the transfer for the payee and your updated backup, committed with OP_RETURN or tapret; invoices with an expiry; `deriveAddress`.
- **Network:** an opt-in `checkWitnesses` lookup on an Esplora server, including the Bitcoin BLAKE2b testnet4. Nothing else uses the network.
- **Tested against** upstream invoice tests, CLI invoices and consignments, every single-byte corruption of those files, payments made by rgbjs, and three payments mined on the BLAKE2b testnet4.

Pins: rgb-cmd 0.11.1 crates (v0.11) and rgb-wallet 0.12.0-rc.3 crates (v0.12). See README.md for known issues.

## 0.0.1 (2025)

A placeholder for the name.
