# v0.12 vectors

- `upstream-invoices.json`: extracted by `scripts/extract-vectors.py` from the tests in `sonic-callreq` 0.12.0 `src/uri.rs` (Apache-2.0, AluVM / RGB-WG).
- `DemoToken.rgb`: a contract consignment from the pinned `rgb-wallet` 0.12.0-rc.3, made offline from the RGB-WG/rgb v0.12.0-rc.3 `examples/` folder: `rgb -d data init`, `rgb -d data import RGB20-FNA.issuer`, `rgb -d data issue -w alice DemoToken.yaml`, `rgb -d data backup -f DemoToken DemoToken.rgb`. The pinned binary's `rgb -d data2 accept -w bob -u DemoToken.rgb` accepts it. The contract ID is `contract:nztjpjRX-_NVDDgd-EHENsqL-AKsGsyV-F6LzljB-X4WTv0Y`, the same one the RGB Docs quick-start shows.

The trusted issuer `rust/v012/issuers/RGB20-FNA.issuer` is copied from RGB-WG/rgb v0.12.0-rc.3 `examples/` (Apache-2.0).
