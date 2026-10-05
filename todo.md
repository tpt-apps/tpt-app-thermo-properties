# TPT Thermo Properties — Build Todo

> Hub listing: `tptsolutions.co.nz/tools/thermo-properties` (registry slug
> `thermo-properties`, category `engineering`). Spec: `apps.md` §T1-6 in the
> tpt-electrician-nz-2 repo. Free WASM edition: single-point property lookup
> for common refrigerants. Paid offline edition ($99 USD, Gumroad): full cycle
> analysis, saved fluid property library, and exportable calculation sheets.
> Solver wraps `tpt-thermodynamics`. UI on `tpt-appfront`.
>
> Hub contract: `wasm-bindgen --target web` glue exposes `mount(container)`
> (called by `WasmAppRunner` after `default()` init). Standalone `trunk serve`
> mounts into the `#tpt-appfront-root` marker div instead — never
> `document.body` — so the two hosts never double-mount.

## Phase 0 — Scaffold & pipeline proof
- [ ] `tpt-appfront init tpt-app-thermo-properties --target dom` (path deps on the tpt-appfront checkout)
- [ ] Restructure scaffold to hub contract: `#[wasm_bindgen] pub fn mount(el)` + `#[wasm_bindgen(start)]` no-op unless `#tpt-appfront-root` present
- [ ] `index.html` carries the `#tpt-appfront-root` marker div
- [ ] Build pipeline script mirroring fea-lite/cutlist-optimizer's (`--target web` glue → web repo `public/apps/thermo-properties/`)
- [ ] Hello-world build loads through the standalone dev page (`trunk serve`)
- [ ] git init + first commit

## Phase 1 — Solver integration (wrap tpt-thermodynamics)
- [ ] Survey `tpt-thermodynamics` for refrigerant/working-fluid property
      correlations (pressure/temperature/enthalpy) and cycle-analysis support
      (refrigeration cycle, Rankine cycle)
- [ ] Adapter crate `engine/`: fluid + state point in → property values out;
      cycle definition in → cycle performance (COP, work, heat duty) out — as
      plain structs, no UI deps, host-unit-testable
- [ ] Validation cases against known reference property tables (e.g. R-134a,
      R-410A at standard points) to establish accuracy bounds
- [ ] Wasm payload size check once the sub-crate set is chosen

## Phase 2 — Free WASM UI
- [ ] Refrigerant picker (common fluids) + single-point property lookup
      (pressure/temperature/enthalpy)
- [ ] Free-tier upsell copy pointing at full cycle analysis, saved library, export
- [ ] Browser-verified: mount, lookup, results render correctly

## Phase 3 — Paid desktop edition (cargo feature `pro`)
- [ ] `pro` gates: full refrigeration/Rankine cycle analysis, saved fluid
      property library, exportable calculation sheets for design documentation
- [ ] Pro UI: cycle input form, cycle diagram/results, saved-calc library,
      export button
- [ ] Desktop shell via `tpt-appfront-webview`; Windows exe build + smoke test
- [ ] Gumroad product from apps.md listing copy ($99), exe upload, URL into
      registry — manual step, listing copy at apps.md §T1-6

## Phase 4 — Ship & measure
- [ ] Registry entry (`engineering` category, `wasm: { entry: '/apps/thermo-properties/thermo-properties.js' }`)
      per the "Adding a finished app to the hub" runbook at the bottom of apps.md
- [ ] Verify hub card/detail/sitemap/runner in dev, then prod after deploy
- [ ] WASM size budget check on the shipped build
