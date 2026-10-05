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
      — **update: the crate set has now landed upstream** (core, data,
      eos-cubic, eos-saft, eos-activity, phase, bubble-dew, flash, transport,
      electrolyte, polymer, umbrella). The engine now delegates the
      Peng-Robinson machinery (cubic solving, residual h/s, root selection)
      to `tpt-thermo-eos-cubic` via path deps, feeding the engine's fluid
      table in through `ComponentDatabase`. The anchor-calibrated
      correlations (psat, liquid density, latent heat, cp) stay here: the
      upstream seed carries only critical constants (no ideal-gas cp
      correlations or saturation anchors) and R-32/R-125/R-1234yf are not in
      the seed yet. PR-1976 kappa is supplied via the pluggable AlphaFunction
      (upstream's default is the SRK kappa). Rankine side unchanged:
      `tpt-eng-props-water` 0.1.0 (IAPWS-IF97 regions 1/2/4).
- [x] Adapter crate `engine/`: fluid + state point in → property values out;
      cycle definition in → cycle performance (COP, work, heat duty) out — as
      plain structs, no UI deps, host-testable
      (`state_at`, `refrigeration_cycle`, `rankine_cycle` (pro))
- [x] Validation cases against known reference property tables (e.g. R-134a,
      R-410A at standard points) to establish accuracy bounds
      — 34 tests green on the delegated engine: per-fluid psat/density/
      latent-heat anchors, R-134a textbook cycle (COP ≈ 5.4), Moran
      8 MPa/480 °C/8 kPa Rankine (η ≈ 0.397, x₂ ≈ 0.794). Superheated h
      moved *closer* to reference tables (R-134a 60 °C/1017 kPa: 441.3 vs
      table ≈ 441.2). Stated bounds ship on every export sheet
      (`export::ACCURACY_NOTE`).
- [x] Wasm payload size check once the sub-crate set is chosen
      — free edition after integration: 256 KB raw, **103.4 KB gzipped**
      (budget ~2 MB gz; +14 KB gz vs the hand-rolled PR — the upstream
      2 718-compound seed dataset dead-strips because it is never
      referenced)

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
- [x] `pro` gates: full refrigeration/Rankine cycle analysis, saved fluid
      property library, exportable calculation sheets for design documentation
      — engine: `refrigeration_cycle` (9 fluids), `rankine_cycle`
      (IAPWS-IF97 via `tpt-eng-props-water`, engine feature `pro`); app:
      cycle cards, P-h diagram canvas, saved-calc library (localStorage),
      Markdown calc-sheet + CSV export, header Pro chip, upsell gated out
- [x] Pro UI: cycle input form, cycle diagram/results, saved-calc library,
      export button
      — browser-verified: refrigeration COP 4.04 @ ηs 0.7 (ideal-cycle
      validation 5.4), Moran Rankine base case 35.7%, P-h diagram with dome/
      isobars/2s marker, library save/load/delete round-trip, sheet+CSV
      export buttons
- [x] Desktop shell via `tpt-appfront-webview`; Windows exe build + smoke test
      — `tpt-thermo-properties-pro.exe` (embedded dist/, single-file),
      launches and stays alive; `build.ps1 -Pro` produces it end-to-end
- [ ] Gumroad product from apps.md listing copy ($249), exe upload, URL into
      registry — manual step, listing copy at apps.md §T1-6
      (**price is $249** per apps.md §T1-6 and GUMROAD.md; the older "$99"
      in this file's header is stale)

## Phase 4 — Ship & measure
- [x] Registry entry (`engineering` category, `wasm: { entry: '/apps/thermo-properties/thermo-properties.js' }`)
      per the "Adding a finished app to the hub" runbook at the bottom of apps.md
      — added to `tpt-apps-registry.ts` with `status: 'coming-soon'`,
      `price: '$249'`, and the proFeatures list; committed to the web repo
      (push + deploy are the user's manual steps)
- [x] Verify hub card/detail/sitemap/runner in dev, then prod after deploy
      — dev-verified on the live Next dev server: /tools card renders with
      the SOON badge and Engineering tag; /tools/thermo-properties mounts
      the app via WasmAppRunner and a lookup (R-717 @ 20 °C → 857.5 kPa,
      the exact reference anchor) runs inside the hub page. Prod check
      pending deploy. (Sitemap picks the entry up on the next production
      build, per `src/app/sitemap.ts`.)
- [x] WASM size budget check on the shipped build
      — free edition 218 KB raw / **89.5 KB gz** (budget ~2 MB gz); pro exe
      1.2 MB single file

### Manual steps remaining (user)
1. Publish the Gumroad product per `GUMROAD.md` (upload
   `target/release/tpt-thermo-properties-pro.exe`, paste the listing copy,
   $249) → copy the product URL.
2. Paste the URL into the `thermo-properties` entry's `gumroadUrl` in
   `tpt-apps-registry.ts` and flip `status` to `'live'`.
3. Commit + push the web repo, deploy to prod, and re-verify the card,
   detail page and runner on tptsolutions.co.nz.
