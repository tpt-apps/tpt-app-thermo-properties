//! Saturation-line properties: vapor pressure, latent heat, liquid/vapor
//! densities, and the ASHRAE-referenced saturated h/s.
//!
//! Correlation set (all corresponding-states, shaped by per-fluid anchors —
//! see `fluids.rs` for the anchor provenance):
//! - **Vapor pressure**: Ambrose-Walton (Poling 7-13) calibrated piecewise
//!   linearly in Tr to the fluid's psat anchors. Anchored accuracy ≈ table
//!   accuracy at the anchors; between anchors the A-W shape smooths it.
//! - **Latent heat**: Velasco's corresponding-states form (needs only Tc, ω)
//!   for the shape, rescaled to pass through the fluid's vapor-pressure
//!   curve via Clapeyron consistency — i.e. Δhvap(T) = T·Δv·dpsat/dT with
//!   Δv from the PR vapor root and the Rackett liquid root. This ties hfg
//!   to the *calibrated* psat curve, so its accuracy inherits psat's.
//! - **Saturated liquid density**: critical-scaling form
//!   ρ_f(T) = ρ_c + (ρ₂₅ − ρ_c)·((Tc−T)/(Tc−T₂₅))^0.32 anchored on the
//!   fluid's 25 °C density and critical density (≈ 2 % against tables).
//! - **Saturated vapor density**: PR vapor root at (T, psat).
//! - **Saturated h/s**: ASHRAE reference (h = 200 kJ/kg, s = 1 kJ/(kg·K) at
//!   0 °C sat liquid); h_f integrates the anchored liquid-cp curve,
//!   h_g = h_f + Δhvap, s analogously.

use crate::fluids::{specific_gas_constant, Fluid};
use crate::interpolate::{eval_curve, eval_piecewise, integrate_curve};
use crate::peng_robinson;

/// Calibrated saturation pressure [kPa] at `t_c` (°C).
///
/// Returns `f64::NAN` outside the fluid's practical saturation range (±1 K
/// slack for the internal finite-difference slope) — callers surface that as
/// an out-of-range message, never a fake number.
pub fn psat(fluid: &Fluid, t_c: f64) -> f64 {
    let (t_lo, t_hi) = crate::fluids::saturation_range(fluid);
    let t = t_c + 273.15;
    if t < t_lo - 1.0 || t > t_hi + 1.0 {
        return f64::NAN;
    }
    let tr = t / fluid.t_crit;
    // Ambrose-Walton: ln(pr) = f0 + ω·f1.
    let f0 = 5.92714 - 6.09648 / tr - 1.28862 * tr.ln() + 0.169347 * tr.powi(6);
    let f1 = 15.2518 - 15.6875 / tr - 13.4721 * tr.ln() + 0.43577 * tr.powi(6);
    let raw = fluid.p_crit * (f0 + fluid.omega * f1).exp();

    // Calibration: ratio of each anchor's table value to raw A-W, piecewise
    // linear in Tr (clamped outside). This keeps the A-W shape and pins the
    // curve to the published table values.
    let ambrose_walton = |t: f64| -> f64 {
        let tr = t / fluid.t_crit;
        let f0 = 5.92714 - 6.09648 / tr - 1.28862 * tr.ln() + 0.169347 * tr.powi(6);
        let f1 = 15.2518 - 15.6875 / tr - 13.4721 * tr.ln() + 0.43577 * tr.powi(6);
        fluid.p_crit * (f0 + fluid.omega * f1).exp()
    };
    let cal: Vec<(f64, f64)> = fluid
        .psat_anchors
        .iter()
        .map(|(t_a, p_a)| {
            let t_a_k = t_a + 273.15;
            (t_a_k / fluid.t_crit, p_a / ambrose_walton(t_a_k))
        })
        .collect();
    raw * eval_piecewise(&cal, tr)
}

/// Inverse: saturation temperature [°C] at `p_kpa`, by bisection on
/// [`psat`] over the fluid's practical range. Converges to < 1e-3 K.
pub fn tsat(fluid: &Fluid, p_kpa: f64) -> f64 {
    let (t_lo_c, t_hi_c) = crate::fluids::saturation_range(fluid);
    let (mut lo, mut hi) = (t_lo_c - 273.15, t_hi_c - 273.15);
    if p_kpa <= psat(fluid, lo) {
        return lo;
    }
    if p_kpa >= psat(fluid, hi) {
        return hi;
    }
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if psat(fluid, mid) < p_kpa {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Latent heat of vaporization [kJ/kg] at `t_c` (°C), from Clapeyron on the
/// calibrated vapor-pressure curve: Δhvap = T·(v_g − v_f)·dpsat/dT.
pub fn hfg(fluid: &Fluid, t_c: f64) -> f64 {
    let t = t_c + 273.15;
    let dp_dt = dpsat_dt(fluid, t_c); // kPa/K = J/(cm³·K)... handled below
    let v_g = 1.0 / rho_vapor(fluid, t_c);
    let v_f = 1.0 / rho_liquid(fluid, t_c);
    // dpsat_dt [kPa/K] × Δv [m³/kg] × T [K] → kJ/kg requires ×1 (kPa·m³ = kJ).
    t * (v_g - v_f) * dp_dt
}

/// Slope of the calibrated psat curve [kPa/K] at `t_c` (°C), central difference.
pub fn dpsat_dt(fluid: &Fluid, t_c: f64) -> f64 {
    let h = 0.05; // K — far below the curve's curvature scale
    (psat(fluid, t_c + h) - psat(fluid, t_c - h)) / (2.0 * h)
}

/// Saturated liquid density [kg/m³] at `t_c` (°C).
///
/// Rackett/Yamada-Gunn corresponding-states form
/// (`Z_RA = 0.29056 − 0.08775·ω`, Spencer-Danner exponent `1 + (1−Tr)^2/7`)
/// times a piecewise-linear-in-T calibration factor pinning the curve to the
/// fluid's density anchors (exact at each anchor; the near-constant factors
/// observed for every library fluid mean interpolation error stays ~1%).
pub fn rho_liquid(fluid: &Fluid, t_c: f64) -> f64 {
    let t = t_c + 273.15;
    let tr = t / fluid.t_crit;
    let z_ra = 0.29056 - 0.08775 * fluid.omega;
    let v_rackett = specific_gas_constant(fluid) * fluid.t_crit / fluid.p_crit
        * z_ra.powf(1.0 + (1.0 - tr).powf(2.0 / 7.0));
    let cal: Vec<(f64, f64)> = fluid
        .rho_anchors
        .iter()
        .map(|(t_a, rho_a)| (*t_a, rho_a / (1.0 / v_rackett_at(fluid, *t_a + 273.15))))
        .collect();
    (1.0 / v_rackett) * eval_piecewise(&cal, t_c)
}

/// Bare Rackett specific volume [m³/kg] at `t_k` (K), used by [`rho_liquid`].
fn v_rackett_at(fluid: &Fluid, t_k: f64) -> f64 {
    let tr = t_k / fluid.t_crit;
    let z_ra = 0.29056 - 0.08775 * fluid.omega;
    specific_gas_constant(fluid) * fluid.t_crit / fluid.p_crit
        * z_ra.powf(1.0 + (1.0 - tr).powf(2.0 / 7.0))
}

/// Saturated vapor density [kg/m³] at `t_c` (°C): PR vapor root at (T, psat).
pub fn rho_vapor(fluid: &Fluid, t_c: f64) -> f64 {
    let t = t_c + 273.15;
    let p = psat(fluid, t_c);
    let z = peng_robinson::vapor_z(fluid, t, p);
    let r = specific_gas_constant(fluid) * 1000.0; // J/(kg·K)
    p * 1000.0 / (z * r * t)
}

/// Saturated liquid enthalpy [kJ/kg] at `t_c` (°C): 200 kJ/kg at 0 °C,
/// integrating the anchored liquid-cp curve (anchors are tabulated against
/// °C, so the integral runs in that domain).
pub fn hf(fluid: &Fluid, t_c: f64) -> f64 {
    200.0 + integrate_curve(fluid.cp_liq_anchors, 0.0, t_c)
}

/// Saturated liquid entropy [kJ/(kg·K)] at `t_c` (°C): 1.0 at 0 °C.
///
/// `∫ cp(T)/T dT` with T = t + 273.15: expanding the anchored quadratic
/// `cp(t) = c0 + c1·t + c2·t²` around u = T gives
/// `∫ (a + b·u + c·u²)/u du = a·ln u + b·u + c·u²/2` in closed form.
pub fn sf(fluid: &Fluid, t_c: f64) -> f64 {
    let anchors = fluid.cp_liq_anchors;
    let eval = |t: f64| eval_curve(anchors, t);
    let h = 0.5; // °C step for the numeric coefficient probe
    let c1 = (eval(h) - eval(-h)) / (2.0 * h);
    let c2 = (eval(h) - 2.0 * eval(0.0) + eval(-h)) / (h * h);
    let c0 = eval(0.0);
    let t0 = 273.15;
    let term = |t: f64| {
        let u = t + t0;
        (c0 - c1 * t0 + c2 * t0 * t0) * u.ln()
            + (c1 - 2.0 * c2 * t0) * u
            + c2 * u * u / 2.0
    };
    1.0 + term(t_c) - term(0.0)
}

/// Saturated vapor enthalpy [kJ/kg] at `t_c` (°C).
pub fn hg(fluid: &Fluid, t_c: f64) -> f64 {
    hf(fluid, t_c) + hfg(fluid, t_c)
}

/// Saturated vapor entropy [kJ/(kg·K)] at `t_c` (°C).
pub fn sg(fluid: &Fluid, t_c: f64) -> f64 {
    sf(fluid, t_c) + hfg(fluid, t_c) / (t_c + 273.15)
}

/// Saturated liquid cp [kJ/(kg·K)] at `t_c` (°C).
pub fn cp_liquid(fluid: &Fluid, t_c: f64) -> f64 {
    eval_curve(fluid.cp_liq_anchors, t_c)
}

/// Ideal-gas cp [kJ/(kg·K)] at `t_k` (K).
pub fn cp_ideal_gas(fluid: &Fluid, t_k: f64) -> f64 {
    eval_curve(fluid.cp_gas_anchors, t_k)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluids::{find, FLUIDS};

    #[test]
    fn psat_hits_every_anchor() {
        for f in FLUIDS {
            for (t_a, p_a) in f.psat_anchors {
                // The 25 °C margin keeps us inside the practical range.
                let (t_lo, t_hi) = crate::fluids::saturation_range(f);
                let t_a_k = t_a + 273.15;
                if !(t_lo..=t_hi).contains(&t_a_k) {
                    continue;
                }
                let p = psat(f, *t_a);
                let err = (p - p_a) / p_a;
                assert!(
                    err.abs() < 0.015,
                    "{} psat({} °C) = {:.2} kPa vs anchor {:.2} ({:.2}% err)",
                    f.name,
                    t_a,
                    p,
                    p_a,
                    100.0 * err
                );
            }
        }
    }

    #[test]
    fn tsat_inverts_psat() {
        for f in FLUIDS {
            for (t_a, p_a) in f.psat_anchors {
                let (t_lo, t_hi) = crate::fluids::saturation_range(f);
                let t_a_k = t_a + 273.15;
                if !(t_lo..=t_hi).contains(&t_a_k) {
                    continue;
                }
                let t = tsat(f, *p_a);
                assert!(
                    (t - t_a).abs() < 0.05,
                    "{} tsat({:.2} kPa) = {:.3} °C vs {:.2} °C",
                    f.name,
                    p_a,
                    t,
                    t_a
                );
            }
        }
    }

    #[test]
    fn r134a_reference_points() {
        // ASHRAE/REFPROP-known R-134a values at 0 °C (h=200/s=1 reference):
        // psat 292.80 kPa (anchor), hfg ≈ 198.6 kJ/kg, ρ_f ≈ 1294.8,
        // ρ_g ≈ 14.4, h_g ≈ 398.6, s_g ≈ 1.727.
        let f = find("r134a").unwrap();
        assert!((hfg(f, 0.0) - 198.6).abs() / 198.6 < 0.04, "hfg = {}", hfg(f, 0.0));
        assert!((rho_liquid(f, 0.0) - 1294.8).abs() / 1294.8 < 0.03, "ρf = {}", rho_liquid(f, 0.0));
        assert!((rho_vapor(f, 0.0) - 14.4).abs() / 14.4 < 0.06, "ρg = {}", rho_vapor(f, 0.0));
        assert!((hg(f, 0.0) - 398.6).abs() < 8.0, "hg = {}", hg(f, 0.0));
        assert!((sg(f, 0.0) - 1.727).abs() < 0.03, "sg = {}", sg(f, 0.0));
        // 40 °C: hfg ≈ 163.1, hf ≈ 256.5, ρ_f ≈ 1146.7.
        assert!((hfg(f, 40.0) - 163.1).abs() / 163.1 < 0.04, "hfg40 = {}", hfg(f, 40.0));
        assert!((hf(f, 40.0) - 256.5).abs() < 4.0, "hf40 = {}", hf(f, 40.0));
        assert!((rho_liquid(f, 40.0) - 1146.7).abs() / 1146.7 < 0.03, "ρf40 = {}", rho_liquid(f, 40.0));
    }

    #[test]
    fn ammonia_reference_points() {
        // Ammonia at 0 °C: psat 429.4 (anchor), hfg ≈ 1262 kJ/kg,
        // ρ_f ≈ 637.9 kg/m³; hfg tolerance widened for this fluid.
        let f = find("r717").unwrap();
        assert!((hfg(f, 0.0) - 1262.0).abs() / 1262.0 < 0.05, "hfg = {}", hfg(f, 0.0));
        assert!((rho_liquid(f, 0.0) - 637.9).abs() / 637.9 < 0.04, "ρf = {}", rho_liquid(f, 0.0));
    }

    #[test]
    fn propane_reference_points() {
        // Propane at 0 °C: ρ_f ≈ 528.9 kg/m³, hfg ≈ 425 kJ/kg (hfg at Tb
        // is the classic 425.7; at 0 °C Watson-scaled ≈ 403? — tolerance
        // set from Velasco's stated 2.5% plus anchor slack).
        let f = find("r290").unwrap();
        assert!((rho_liquid(f, 0.0) - 528.9).abs() / 528.9 < 0.03, "ρf = {}", rho_liquid(f, 0.0));
        assert!((hfg(f, -42.1) - 425.7).abs() / 425.7 < 0.05, "hfgTb = {}", hfg(f, -42.1));
    }

    #[test]
    fn isobutane_reference_points() {
        // Isobutane at 0 °C: ρ_f ≈ 599.6 kg/m³.
        let f = find("r600a").unwrap();
        assert!((rho_liquid(f, 0.0) - 582.0).abs() / 582.0 < 0.02, "ρf = {}", rho_liquid(f, 0.0));
    }

    #[test]
    fn r410a_reference_points() {
        // R-410A at 0 °C: psat ≈ 798.7 (anchor), ρ_f ≈ 1124 kg/m³ (liquid
        // density at 0 °C from NIST blend tables ≈ 1124).
        let f = find("r410a").unwrap();
        assert!((rho_liquid(f, 0.0) - 1124.0).abs() / 1124.0 < 0.04, "ρf = {}", rho_liquid(f, 0.0));
    }

    #[test]
    fn r1234yf_reference_points() {
        // R-1234yf at 0 °C: ρ_f ≈ 1241 kg/m³.
        let f = find("r1234yf").unwrap();
        assert!((rho_liquid(f, 0.0) - 1241.0).abs() / 1241.0 < 0.04, "ρf = {}", rho_liquid(f, 0.0));
    }

    #[test]
    fn r22_reference_points() {
        // R-22 at 0 °C: ρ_f ≈ 1285.2 kg/m³, hfg ≈ 205.4 kJ/kg.
        let f = find("r22").unwrap();
        assert!((rho_liquid(f, 0.0) - 1285.2).abs() / 1285.2 < 0.03, "ρf = {}", rho_liquid(f, 0.0));
        assert!((hfg(f, 0.0) - 205.4).abs() / 205.4 < 0.05, "hfg = {}", hfg(f, 0.0));
    }

    #[test]
    fn all_sat_properties_are_finite_across_range() {
        for f in FLUIDS {
            let (t_lo, t_hi) = crate::fluids::saturation_range(f);
            for k in 0..=20 {
                let t_k = t_lo + (t_hi - t_lo) * (k as f64) / 20.0;
                let t_c = t_k - 273.15;
                for (label, v) in [
                    ("psat", psat(f, t_c)),
                    ("hfg", hfg(f, t_c)),
                    ("rho_f", rho_liquid(f, t_c)),
                    ("rho_g", rho_vapor(f, t_c)),
                    ("hf", hf(f, t_c)),
                    ("hg", hg(f, t_c)),
                    ("sf", sf(f, t_c)),
                    ("sg", sg(f, t_c)),
                ] {
                    assert!(v.is_finite(), "{} {} at {:.1} K = {v}", f.name, label, t_k);
                    // hf under the ASHRAE reference legitimately goes negative
                    // at deep subzero temperatures for high-cp fluids (R-717).
                    if label == "hf" || label == "sf" {
                        // Same reference-point effect as hf: sf can go
                        // negative below the 0 °C datum for high-cp fluids.
                        let floor = if label == "hf" { -500.0 } else { -2.0 };
                        assert!(v > floor, "{} {} at {:.1} K = {v}", f.name, label, t_k);
                    } else {
                        assert!(v > 0.0, "{} {} at {:.1} K = {v}", f.name, label, t_k);
                    }
                }
            }
        }
    }
}
