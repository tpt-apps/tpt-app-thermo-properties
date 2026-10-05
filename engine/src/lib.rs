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
//! Integration with the `tpt-thermodynamics` workspace (path deps — not yet
//! on crates.io): the Peng-Robinson cubic solver and the residual h/s terms
//! now come from `tpt-thermo-eos-cubic` (see `peng_robinson.rs`), with the
//! engine's fluid table fed in through the `ComponentDatabase` trait. The
//! anchor-calibrated correlations (psat, saturated liquid density, latent
//! heat, heat capacities) stay in this crate: the upstream compound seed
//! carries only critical constants — no ideal-gas cp correlations or
//! saturation anchors — and three of the nine fluids (R-32, R-125, R-1234yf)
//! are not in the seed yet. Contributing those records upstream is the
//! natural follow-up.

pub mod cycle;
pub mod fluids;
pub mod interpolate;
pub mod peng_robinson;
pub mod saturation;
pub mod state;

#[cfg(feature = "pro")]
pub mod export;
