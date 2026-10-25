# v0.11 vectors

- `upstream-invoices.json`: extracted by `scripts/extract-vectors.py` from the tests in `rgb-invoicing` 0.11.1 `src/parse.rs` (Apache-2.0, rgb-protocol).
- `demo.contract.rgb`: a NIA contract consignment from the pinned `rgb-cmd` 0.11.1, made offline: `rgb -n regtest import NonInflatableAsset.rgb`, `rgb -n regtest issue ssi:rgbjs-test demo.yaml`, `rgb export <id> demo.contract.rgb`. The CLI reported the contract ID `rgb:v4Kpkuwn-yjzKgYa-5G5h0cc-lLVyr2B-hdpCTaR-KOe~agc`. Its genesis seal is the mainnet genesis coinbase output: a test vector, not a real asset.
- `nia_transfer.v0112.rgb`: a regtest transfer from rgb-api master tests (`cli/tests/fixtures/nia_transfer.rgb`, Apache-2.0). It is in the 0.11.2 consignment format, which 0.11.1 can't read, so the tests expect a clear decode error.
