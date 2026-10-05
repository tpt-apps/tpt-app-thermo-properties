//! Single-point thermodynamic states: (T, p) lookups, two-phase quality, and
//! the entropy-driven inversions the cycle analysis needs.

use crate::fluids::{specific_gas_constant, Fluid};
use crate::interpolate::{eval_curve, integrate_curve, integrate_curve_over_t};
use crate::peng_robinson;
use crate::saturation;

/// A complete single-point state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct State {
    /// Temperature [°C].
    pub t_c: f64,
    /// Pressure [kPa].
    pub p_kpa: f64,
    /// Specific enthalpy [kJ/kg].
    pub h: f64,
    /// Specific entropy [kJ/(kg·K)].
    pub s: f64,
    /// Density [kg/m³].
    pub rho: f64,
    /// Region the state was evaluated in.
    pub region: Region,
    /// Vapor mass fraction in the two-phase region (0 elsewhere).
    pub quality: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    SubcooledLiquid,
    Saturation,
    SuperheatedVapor,
}

impl Region {
    pub fn label(&self) -> &'static str {
        match self {
            Region::SubcooledLiquid => "subcooled liquid",
            Region::Saturation => "two-phase (saturation)",
            Region::SuperheatedVapor => "superheated vapor",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LookupError(pub String);

impl std::fmt::Display for LookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Full state at `(t_c [°C], p_kpa [kPa])` — the free tier's lookup.
///
/// Region selection against the calibrated saturation line:
/// - `p > psat(T)` → subcooled liquid (anchored hf + constant-v pressure term),
/// - `p < psat(T)` → superheated vapor (saturation-anchored hg/sf + ideal-gas
///   sensible heat + PR residual correction),
/// - on the line → two-phase at the (T, p) saturation point (quality 1.0 for
///   the saturated-vapor convention; use [`quality_from_enthalpy`] for
///   arbitrary two-phase states).
pub fn state_at(fluid: &Fluid, t_c: f64, p_kpa: f64) -> Result<State, LookupError> {
    if !t_c.is_finite() || !p_kpa.is_finite() {
        return Err(LookupError("Temperature and pressure must be finite numbers.".into()));
    }
    if p_kpa <= 0.0 {
        return Err(LookupError("Pressure must be positive.".into()));
    }
    let psat = saturation::psat(fluid, t_c);
    if psat.is_nan() {
        return Err(LookupError(format!(
            "{} °C is outside {}'s saturation range (correlations are valid from {:.0} °C to the critical point).",
            t_c,
            fluid.name,
            crate::fluids::saturation_range(fluid).0 - 273.15
        )));
    }
    let t = t_c + 273.15;

    if p_kpa > psat * 1.0005 {
        // Subcooled (compressed) liquid: saturation-anchored h/s plus the
        // constant-liquid-density pressure correction.
        let hf = saturation::hf(fluid, t_c);
        let rho_f = saturation::rho_liquid(fluid, t_c);
        let dp = p_kpa - psat; // kPa; kPa·m³/kg = kJ/kg
        let h = hf + dp / rho_f;
        let s = saturation::sf(fluid, t_c);
        Ok(State { t_c, p_kpa, h, s, rho: rho_f, region: Region::SubcooledLiquid, quality: 0.0 })
    } else if p_kpa < psat * 0.9995 {
        // Superheated vapor.
        let t_sat_c = saturation::tsat(fluid, p_kpa);
        let t_sat = t_sat_c + 273.15;
        let hg = saturation::hg(fluid, t_sat_c);
        let sg = saturation::sg(fluid, t_sat_c);
        // Sensible heat from Tsat(p) to T (ideal-gas cp integral)…
        let dh = integrate_curve(fluid.cp_gas_anchors, t_sat, t);
        let ds = integrate_curve_over_t(fluid.cp_gas_anchors, t_sat, t);
        // …plus the PR residual difference between this state and the
        // saturated-vapor anchor, so h/s stay consistent as pressure moves.
        let psat_t_sat = saturation::psat(fluid, t_sat_c);
        let hr_state = peng_robinson::residual_enthalpy(fluid, t, p_kpa);
        let hr_sat = peng_robinson::residual_enthalpy(fluid, t_sat, psat_t_sat);
        let sr_state = peng_robinson::residual_entropy(fluid, t, p_kpa);
        let sr_sat = peng_robinson::residual_entropy(fluid, t_sat, psat_t_sat);
        let h = hg + dh + (hr_state - hr_sat);
        let s = sg + ds - (specific_gas_constant(fluid) * (p_kpa / psat_t_sat).ln())
            + (sr_state - sr_sat);
        let z = peng_robinson::vapor_z(fluid, t, p_kpa);
        let rho = p_kpa * 1000.0 / (z * specific_gas_constant(fluid) * 1000.0 * t);
        Ok(State {
            t_c,
            p_kpa,
            h,
            s,
            rho,
            region: Region::SuperheatedVapor,
            quality: 1.0,
        })
    } else {
        // On the saturation line: report the saturated-vapor state.
        let rho_g = saturation::rho_vapor(fluid, t_c);
        Ok(State {
            t_c,
            p_kpa: psat,
            h: saturation::hg(fluid, t_c),
            s: saturation::sg(fluid, t_c),
            rho: rho_g,
            region: Region::Saturation,
            quality: 1.0,
        })
    }
}

/// Two-phase quality from enthalpy at saturation temperature `t_sat_c`.
pub fn quality_from_enthalpy(fluid: &Fluid, t_sat_c: f64, h: f64) -> f64 {
    let hf = saturation::hf(fluid, t_sat_c);
    let hg = saturation::hg(fluid, t_sat_c);
    ((h - hf) / (hg - hf)).clamp(0.0, 1.0)
}

/// Two-phase mixture state at quality `x` and saturation temperature `t_sat_c`.
pub fn two_phase_state(fluid: &Fluid, t_sat_c: f64, x: f64) -> State {
    let hf = saturation::hf(fluid, t_sat_c);
    let hg = saturation::hg(fluid, t_sat_c);
    let rho_f = saturation::rho_liquid(fluid, t_sat_c);
    let rho_g = saturation::rho_vapor(fluid, t_sat_c);
    State {
        t_c: t_sat_c,
        p_kpa: saturation::psat(fluid, t_sat_c),
        h: hf + x * (hg - hf),
        s: saturation::sf(fluid, t_sat_c) + x * (saturation::sg(fluid, t_sat_c) - saturation::sf(fluid, t_sat_c)),
        rho: 1.0 / (x / rho_g + (1.0 - x) / rho_f),
        region: Region::Saturation,
        quality: x,
    }
}

/// Superheated state at entropy `s` and pressure `p_kpa` (vapor region only):
/// the compressor-outlet / turbine-outlet inversion. Bisection on
/// temperature; errors if `s` is below the saturated-vapor entropy (the
/// state is two-phase — callers use [`two_phase_state`] instead).
pub fn superheated_from_entropy(
    fluid: &Fluid,
    p_kpa: f64,
    s: f64,
) -> Result<State, LookupError> {
    let t_sat_c = saturation::tsat(fluid, p_kpa);
    if s <= saturation::sg(fluid, t_sat_c) {
        return Err(LookupError(
            "state is at or below the saturation line at this pressure".into(),
        ));
    }
    // Upper bracket: cp_ig/T ≥ ~0.6 K⁻¹·(kJ/kg·K)⁻¹ for all library fluids,
    // so 400 K of superheat always brackets the solution.
    let mut lo = t_sat_c;
    let mut hi = t_sat_c + 400.0;
    let s_at = |t_c: f64| superheated_s(fluid, t_c, p_kpa, t_sat_c);
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if s_at(mid) < s {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let t_c = 0.5 * (lo + hi);
    let h = superheated_h(fluid, t_c, p_kpa, t_sat_c);
    let z = peng_robinson::vapor_z(fluid, t_c + 273.15, p_kpa);
    let rho = p_kpa * 1000.0 / (z * specific_gas_constant(fluid) * 1000.0 * (t_c + 273.15));
    Ok(State { t_c, p_kpa, h, s, rho, region: Region::SuperheatedVapor, quality: 1.0 })
}

fn superheated_h(fluid: &Fluid, t_c: f64, p_kpa: f64, t_sat_c: f64) -> f64 {
    let t = t_c + 273.15;
    let t_sat = t_sat_c + 273.15;
    let psat_t_sat = saturation::psat(fluid, t_sat_c);
    let dh = integrate_curve(fluid.cp_gas_anchors, t_sat, t);
    let hr_state = peng_robinson::residual_enthalpy(fluid, t, p_kpa);
    let hr_sat = peng_robinson::residual_enthalpy(fluid, t_sat, psat_t_sat);
    saturation::hg(fluid, t_sat_c) + dh + (hr_state - hr_sat)
}

fn superheated_s(fluid: &Fluid, t_c: f64, p_kpa: f64, t_sat_c: f64) -> f64 {
    let t = t_c + 273.15;
    let t_sat = t_sat_c + 273.15;
    let psat_t_sat = saturation::psat(fluid, t_sat_c);
    let ds = integrate_curve_over_t(fluid.cp_gas_anchors, t_sat, t);
    let sr_state = peng_robinson::residual_entropy(fluid, t, p_kpa);
    let sr_sat = peng_robinson::residual_entropy(fluid, t_sat, psat_t_sat);
    saturation::sg(fluid, t_sat_c) + ds
        - specific_gas_constant(fluid) * (p_kpa / psat_t_sat).ln()
        + (sr_state - sr_sat)
}

/// Superheated state at `(t_c, p_kpa)` — internal helper reused by cycles
/// (assumes validity was checked by [`state_at`]).
pub fn superheated_state(fluid: &Fluid, t_c: f64, p_kpa: f64) -> State {
    let t_sat_c = saturation::tsat(fluid, p_kpa);
    let t = t_c + 273.15;
    let h = superheated_h(fluid, t_c, p_kpa, t_sat_c);
    let s = superheated_s(fluid, t_c, p_kpa, t_sat_c);
    let z = peng_robinson::vapor_z(fluid, t, p_kpa);
    let rho = p_kpa * 1000.0 / (z * specific_gas_constant(fluid) * 1000.0 * t);
    State { t_c, p_kpa, h, s, rho, region: Region::SuperheatedVapor, quality: 1.0 }
}

/// Saturated-liquid cp at the *given temperature*, exposed for the UI's
/// fluid card (not part of lookups).
pub fn cp_liq_at(fluid: &Fluid, t_c: f64) -> f64 {
    eval_curve(fluid.cp_liq_anchors, t_c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluids::{find, FLUIDS};

    #[test]
    fn region_selection_brackets_saturation() {
        let f = find("r134a").unwrap();
        let sat = state_at(f, 0.0, 292.8).unwrap();
        assert_eq!(sat.region, Region::Saturation);
        let liq = state_at(f, 0.0, 500.0).unwrap();
        assert_eq!(liq.region, Region::SubcooledLiquid);
        assert!(liq.h > saturation::hf(f, 0.0));
        let vap = state_at(f, 20.0, 292.8).unwrap();
        assert_eq!(vap.region, Region::SuperheatedVapor);
        assert!(vap.h > saturation::hg(f, 0.0));
    }

    #[test]
    fn r134a_superheated_anchor() {
        // R-134a at 20 °C, 292.8 kPa (20 K superheat at 0 °C saturation).
        // The real-gas isobaric path from the saturated-vapor anchor carries
        // an effective cp between the ideal-gas cp (~0.85 kJ/kg·K) and the
        // near-saturation enhanced value (~1.0 kJ/kg·K) — bound the result
        // there rather than pinning a single table digit.
        let f = find("r134a").unwrap();
        let st = state_at(f, 20.0, 292.8).unwrap();
        let hg0 = saturation::hg(f, 0.0);
        assert!(
            st.h > hg0 + 0.78 * 20.0 && st.h < hg0 + 1.12 * 20.0,
            "h = {:.2} outside cp-bounded [{:.2}, {:.2}]",
            st.h,
            hg0 + 0.78 * 20.0,
            hg0 + 1.12 * 20.0
        );
        assert!((st.s - 1.79).abs() < 0.03, "s = {:.4}", st.s);
    }

    #[test]
    fn entropy_inversion_round_trips() {
        for f in FLUIDS {
            // Pick a representative superheated state per fluid and check
            // state_at → superheated_from_entropy round-trips.
            let t_sat_c = 0.0f64.max(crate::fluids::saturation_range(f).0 - 273.15 + 5.0);
            let p = saturation::psat(f, t_sat_c);
            let start = superheated_state(f, t_sat_c + 35.0, p);
            let back = superheated_from_entropy(f, p, start.s).unwrap();
            assert!(
                (back.t_c - start.t_c).abs() < 0.05 && (back.h - start.h).abs() < 0.05,
                "{} round trip: {:.3}→{:.3} °C, h {:.2}→{:.2}",
                f.name,
                start.t_c,
                back.t_c,
                start.h,
                back.h
            );
        }
    }

    #[test]
    fn two_phase_quality_boundaries() {
        let f = find("r134a").unwrap();
        assert!((quality_from_enthalpy(f, 0.0, saturation::hf(f, 0.0)) - 0.0).abs() < 1e-9);
        assert!((quality_from_enthalpy(f, 0.0, saturation::hg(f, 0.0)) - 1.0).abs() < 1e-9);
        let mix = two_phase_state(f, 0.0, 0.4);
        assert_eq!(mix.region, Region::Saturation);
        assert!((mix.quality - 0.4).abs() < 1e-9);
        assert!(mix.rho > saturation::rho_vapor(f, 0.0)); // denser than pure vapor
    }

    #[test]
    fn out_of_range_inputs_error_cleanly() {
        let f = find("r134a").unwrap();
        assert!(state_at(f, -100.0, 100.0).is_err());
        assert!(state_at(f, 0.0, -5.0).is_err());
        assert!(state_at(f, 0.0, 0.0).is_err());
        assert!(state_at(f, f64::NAN, 100.0).is_err());
    }

    #[test]
    fn thermo_consistency_across_fluids() {
        // ds along a superheated isobar must be positive; dh likewise.
        for f in FLUIDS {
            let (t_lo, _) = crate::fluids::saturation_range(f);
            let t_sat_c = (t_lo - 273.15).max(0.0).min(60.0);
            let p = saturation::psat(f, t_sat_c);
            let a = superheated_state(f, t_sat_c + 20.0, p);
            let b = superheated_state(f, t_sat_c + 40.0, p);
            assert!(b.h > a.h, "{} dh>0", f.name);
            assert!(b.s > a.s, "{} ds>0", f.name);
            // cp estimate from the two points must be near the anchor curve.
            let cp_est = (b.h - a.h) / 20.0;
            let cp_anchor = eval_curve(f.cp_gas_anchors, (a.t_c + b.t_c) / 2.0 + 273.15);
            assert!(
                (cp_est - cp_anchor).abs() / cp_anchor < 0.25,
                "{} cp_est {:.3} vs anchor {:.3}",
                f.name,
                cp_est,
                cp_anchor
            );
        }
    }
}
