//! tpt-thermo-engine — the solver behind TPT Thermo Properties.
//!
//! Refrigerant property lookups and (pro) thermodynamic cycle analysis as a
//! pure, UI-free, host-testable crate:
//!
//! - [`fluids`] — the built-in fluid library (anchor-table data),
//! - [`saturation`] — saturation-line properties,
//! - [`state`] — single-point (T, p) lookups and entropy inversions,
//! - [`peng_robinson`] — vapor compressibility + residual h/s,
//! - [`interpolate`] — the anchor-curve helpers everything above builds on,
//! - [`cycle`] (feature `pro`) — refrigeration and Rankine cycle analysis,
//! - [`export`] (feature `pro`) — calculation-sheet rendering.
//!
//! The long-term home for these correlations is the `tpt-thermodynamics`
//! workspace; until that repo's crate set lands, this crate owns the
//! implementation behind the same public surface (fluid + state in → values
//! out; cycle definition in → performance out), so a later swap is an
//! engine-internal change, not an app change.

pub mod cycle;
pub mod fluids;
pub mod interpolate;
pub mod peng_robinson;
pub mod saturation;
pub mod state;

#[cfg(feature = "pro")]
pub mod export;
