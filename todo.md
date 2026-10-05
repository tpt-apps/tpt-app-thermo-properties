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
- [x] `tpt-appfront init tpt-app-thermo-properties --target dom` (path deps on the tpt-appfront checkout)
      (hand-scaffolded mirroring tpt-app-fea-lite, which is the product of that init)
- [x] Restructure scaffold to hub contract: `#[wasm_bindgen] pub fn mount(el)` + `#[wasm_bindgen(start)]` no-op unless `#tpt-appfront-root` present
- [x] `index.html` carries the `#tpt-appfront-root` marker div
- [x] Build pipeline script mirroring fea-lite/cutlist-optimizer's (`--target web` glue → web repo `public/apps/thermo-properties/`)
- [x] Hello-world build loads through the standalone dev page (`trunk serve`, browser-verified at 127.0.0.1)
- [x] git init + first commit

## Phase 1 — Solver integration (wrap tpt-thermodynamics)
- [x] Survey `tpt-thermodynamics` for refrigerant/working-fluid property
      correlations (pressure/temperature/enthalpy) and cycle-analysis support
      (refrigeration cycle, Rankine cycle)
      — **finding: that repo is workspace bootstrap only, zero crates built
      (its own todo estimates 16-18 sessions)**. Correlations therefore live
      in this repo's `engine/` behind the same public surface (fluid + state
      in → values out; cycle in → performance out), so a later
      tpt-thermodynamics swap is engine-internal. Rankine side uses published
      `tpt-eng-props-water` 0.1.0 (IAPWS-IF97 regions 1/2/4).
- [x] Adapter crate `engine/`: fluid + state point in → property values out;
      cycle definition in → cycle performance (COP, work, heat duty) out — as
      plain structs, no UI deps, host-testable
      (`state_at`, `refrigeration_cycle`, `rankine_cycle` (pro))
- [x] Validation cases against known reference property tables (e.g. R-134a,
      R-410A at standard points) to establish accuracy bounds
      — 34 tests: per-fluid psat/density/latent-heat anchors, R-134a
      textbook cycle (COP ≈ 5.4), Moran 8 MPa/480 °C/8 kPa Rankine
      (η ≈ 0.397, x₂ ≈ 0.794). Stated bounds ship on every export sheet
      (`export::ACCURACY_NOTE`).
- [x] Wasm payload size check once the sub-crate set is chosen
      — free edition: 150 KB raw, **57.3 KB gzipped** (budget ~2 MB gz)

## Phase 2 — Free WASM UI
- [x] Refrigerant picker (common fluids) + single-point property lookup
      (pressure/temperature/enthalpy)
      — 9 fluids × 3 modes (saturation by T, saturation by p, superheated
      (T, p) point): full sat table (psat, ρf/ρg, hf/hg/hfg, sf/sg), point
      table (h, s, ρ, v, region), fluid-constants card, ASHRAE reference
      stated in the header. Browser-verified through trunk serve: all three
      modes, invalid/out-of-range inputs, example button.
- [x] Free-tier upsell copy pointing at full cycle analysis, saved library, export
- [x] Browser-verified: mount, lookup, results render correctly
      (bundle: 218 KB raw, **89.5 KB gz**; copied to the web repo's
      `public/apps/thermo-properties/`)

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
