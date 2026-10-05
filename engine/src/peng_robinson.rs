//! Peng-Robinson equation of state: vapor-root compressibility plus residual
//! (departure) enthalpy and entropy.
//!
//! Used for:
//! - saturated/superheated **vapor density** (better than ideal-gas in the
//!   pressure range refrigeration cycles actually run at),
//! - the vapor-side enthalpy/entropy **consistency correction** so a state's
//!   h and s are anchored on the saturation curve (Watson latent heat) and
//!   shift correctly with superheat and pressure.
//!
//! Accuracy away from the critical point is a few percent on density; the
//! residual terms are small corrections on top of saturation-anchored h/s.
//! Everything here is closed-form in Z, solved from the standard cubic.

use crate::fluids::{specific_gas_constant, Fluid};

/// PR constants at one (T, p) state.
struct PrPoint {
    z: f64,
    /// B = b·p/(R·T) (dimensionless).
    b: f64,
    /// κ·√Tr·[1 + κ(1 − √Tr)] — the T·dα/dT factor of the departure terms.
    t_dalpha: f64,
}

/// Evaluates PR at `(t [K], p [kPa])`, returning the **vapor** (largest-real)
/// compressibility root.
fn vapor_point(fluid: &Fluid, t: f64, p_kpa: f64) -> PrPoint {
    let r = specific_gas_constant(fluid) * 1000.0; // J/(kg·K)
    let tr = t / fluid.t_crit;
    let sqrt_tr = tr.sqrt();
    let kappa = 0.37464 + 1.54226 * fluid.omega - 0.26992 * fluid.omega * fluid.omega;
    // α(T) = [1 + κ(1 − √Tr)]²  (the a·α attraction term's temperature factor)
    let alpha = (1.0 + kappa * (1.0 - sqrt_tr)).powi(2);
    // T·dα/dT with dα/dT = κ[1+κ(1−√Tr)]/√(T·Tc):
    let t_dalpha = kappa * sqrt_tr * (1.0 + kappa * (1.0 - sqrt_tr));

    let p_pa = p_kpa * 1000.0;
    // a = 0.45724·R²Tc²/Pc, b = 0.07780·R·Tc/Pc (per-mass form; the a/R²T²
    // and b/RT reductions below make the per-mass scale cancel anyway).
    let a_coef = 0.45724 * r * r * fluid.t_crit * fluid.t_crit / (fluid.p_crit * 1000.0);
    let b_coef = 0.07780 * r * fluid.t_crit / (fluid.p_crit * 1000.0);

    let big_a = a_coef * alpha * p_pa / (r * r * t * t);
    let big_b = b_coef * p_pa / (r * t);

    // Z³ − (1−B)Z² + (A − 3B² − 2B)Z − (AB − B² − B³) = 0
    let c2 = -(1.0 - big_b);
    let c1 = big_a - 3.0 * big_b * big_b - 2.0 * big_b;
    let c0 = -(big_a * big_b - big_b * big_b - big_b * big_b * big_b);
    let z = largest_real_root(c2, c1, c0);

    PrPoint { z, b: big_b, t_dalpha }
}

/// Largest real root of x³ + c2·x² + c1·x + c0 = 0 on (0, 1], found by
/// scanning downward from 1.0 for the first sign change and bisecting inside
/// that step. The vapor root always sits in (1−B, 1] with f(1) = A > 0, so
/// this never needs a convergence guess; 60 scan steps + 60 bisections give
/// full f64 precision.
fn largest_real_root(c2: f64, c1: f64, c0: f64) -> f64 {
    let f = |z: f64| ((z + c2) * z + c1) * z + c0;
    let steps = 60;
    let mut hi = 1.0;
    for k in 1..=steps {
        let z = 1.0 - (k as f64) / (steps as f64);
        if f(z) < 0.0 {
            // Sign change inside (z, hi]: bisect.
            let mut lo = z;
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                if f(mid) < 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return 0.5 * (lo + hi);
        }
        hi = z;
    }
    // No sign change below 1.0 → the root is 1.0 itself (A → 0 limit).
    1.0
}

/// Compressibility factor of the vapor at `(t [K], p [kPa])`.
pub fn vapor_z(fluid: &Fluid, t: f64, p_kpa: f64) -> f64 {
    vapor_point(fluid, t, p_kpa).z
}

/// Residual enthalpy h^R [kJ/kg] at `(t [K], p [kPa])` (vapor root).
///
/// h^R = R·T·(Z−1) + (T·da/dT − a)/(2√2·b)·ln[(Z+(1+√2)B)/(Z+(1−√2)B)],
/// expressed through the dimensionless A, B and T·dα/dT factors.
pub fn residual_enthalpy(fluid: &Fluid, t: f64, p_kpa: f64) -> f64 {
    let pt = vapor_point(fluid, t, p_kpa);
    let r = specific_gas_constant(fluid);
    let log_term = ((pt.z + (1.0 + 2f64.sqrt()) * pt.b)
        / (pt.z + (1.0 - 2f64.sqrt()) * pt.b))
    .abs()
    .ln();
    // H^R/(R·T) = Z − 1 + (T·dα/dT − α)/(2√2·B)·ln(…); α recomputed inline.
    let tr = t / fluid.t_crit;
    let kappa = 0.37464 + 1.54226 * fluid.omega - 0.26992 * fluid.omega * fluid.omega;
    let alpha = (1.0 + kappa * (1.0 - tr.sqrt())).powi(2);
    r * t * (pt.z - 1.0 + (pt.t_dalpha - alpha) / (2.0 * 2f64.sqrt() * pt.b) * log_term) / 1000.0
}

/// Residual entropy s^R [kJ/(kg·K)] at `(t [K], p [kPa])` (vapor root).
///
/// s^R/R = ln(Z − B) + (dα·T factor)/(2√2·B)·ln(…)
pub fn residual_entropy(fluid: &Fluid, t: f64, p_kpa: f64) -> f64 {
    let pt = vapor_point(fluid, t, p_kpa);
    let r = specific_gas_constant(fluid);
    let log_term = ((pt.z + (1.0 + 2f64.sqrt()) * pt.b)
        / (pt.z + (1.0 - 2f64.sqrt()) * pt.b))
    .abs()
    .ln();
    r * ((pt.z - pt.b).abs().max(1e-15).ln()
        + pt.t_dalpha / (2.0 * 2f64.sqrt() * pt.b) * log_term)
        / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluids::{find, FLUIDS};

    #[test]
    fn vapor_z_tends_to_one_at_low_pressure() {
        let r134a = find("r134a").unwrap();
        // 1 kPa, 300 K: essentially ideal gas.
        let z = vapor_z(r134a, 300.0, 1.0);
        assert!((z - 1.0).abs() < 0.002, "Z = {z}");
    }

    #[test]
    fn vapor_z_departs_at_saturation() {
        let r134a = find("r134a").unwrap();
        // Saturated vapor at 0 °C (292.8 kPa): vg ≈ 0.0694 m³/kg
        // (ASHRAE tables) → Z = pv/(RT) = 292 800·0.0694/(188.85·273.15) ≈ 0.9687.
        let z = vapor_z(r134a, 273.15, 292.8);
        let expected = 292_800.0 * 0.0694 / (specific_gas_constant(r134a) * 1000.0 * 273.15);
        assert!((z - expected).abs() < 0.02, "Z = {z}, table-implied {expected}");
    }

    #[test]
    fn residual_enthalpy_is_small_away_from_saturation() {
        // At low reduced pressure the residual enthalpy of a superheated
        // vapor is a small fraction of the ideal-gas sensible heat.
        for f in FLUIDS {
            let hr = residual_enthalpy(f, 400.0, 100.0);
            assert!(hr.abs() < 12.0, "{} hR at 400 K/100 kPa = {hr}", f.name);
        }
    }
}
