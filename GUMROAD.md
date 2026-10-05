# Gumroad listing

Everything needed to create/update the Gumroad product once this app is
built. Source copy is `apps.md` §T1-6 in the `tpt-electrician-nz-2` repo — if
that changes, update here too (and vice versa).

## Step-by-step: publish this listing

1. Build the Pro edition (`./build.ps1 -Pro`, once the build pipeline exists
   — see `todo.md` and mirror `tpt-app-fea-lite`/`tpt-app-cutlist-optimizer`'s
   pattern) — produces the packaged exe/zip in `release/`.
2. Log in to Gumroad → **New product** → **Digital product**.
3. Set the product name, price, and upload the packaged file (all below).
4. Paste the short description, bullets, and full description (below) into
   their respective fields.
5. Set category/tags (below) and add a cover image (see note below).
6. Publish, then copy the resulting product URL (`https://gum.co/...`).
7. Paste that URL into the `thermo-properties` entry's `gumroadUrl` field in
   `tpt-electrician-nz-2\src\lib\tpt-apps-registry.ts`, and set `price: '$249'`
   on the same entry.
8. Flip that entry's `status` to `'live'` once both the free demo and this
   Gumroad listing exist.
9. Commit and push the registry repo.

## File to upload

Not built yet. Once packaged, this will be the Pro exe/zip produced by the
app's `build.ps1 -Pro` (or equivalent) — see `todo.md`.

## Product name

```
TPT Thermo Properties — Refrigerant Property & Cycle Calculator
```

## Price

**$249 USD, one-time purchase** (no subscription).

## Short description / summary field

```
Property lookups and cycle analysis for engineers designing HVAC, refrigeration, and process systems — without a REFPROP+Aspen budget.
```

## Bullets / feature list

```
Refrigerant property lookup
Refrigeration/Rankine cycle analysis
Exportable design calc sheets
One-time purchase
For designers, not installers
```

## Full description (long-form field)

```
TPT Thermo Properties provides refrigerant/working-fluid property lookups
(pressure/temperature/enthalpy) and thermodynamic cycle analysis
(refrigeration cycle, Rankine cycle) for engineers sizing or checking a
system design.

Who it's for: mechanical and process engineers **designing**
HVAC/refrigeration/process systems — an explicitly professional-engineering
audience, not the tradies installing the resulting systems. NIST's REFPROP
plus Aspen HYSYS-class tools are the incumbents, and their licensing/training
cost puts them out of reach for a small consultancy doing occasional cycle
analysis.

Pro edition (this download):
- Full cycle analysis (refrigeration + Rankine)
- Saved fluid property library
- Exportable calculation sheets for design documentation

Try the free single-point property lookup first, right in your browser, no
install: tptsolutions.co.nz/tools/thermo-properties

Requirements: Windows 10 (20H2+) or Windows 11. Uses the WebView2 runtime,
preinstalled on both — nothing extra to download.
```

## Category / tags

- Category: **Software > Design & Engineering** (or Gumroad's closest equivalent)
- Tags: `thermodynamics`, `refrigeration-engineering`, `hvac-design`,
  `process-engineering`, `engineering-software`, `windows`

## Cover image / thumbnail

Not created yet. A screenshot of a full cycle analysis (P-h diagram or
similar) is the most convincing cover once built. No stock/AI art.

## After publishing

1. Copy the resulting product URL (`https://gum.co/...`).
2. Paste it into the `gumroadUrl` field of the `thermo-properties` entry in
   `tpt-electrician-nz-2\src\lib\tpt-apps-registry.ts`, and set `price: '$249'`.
3. Set `status` to `'live'`.
4. Commit and push the registry repo.
