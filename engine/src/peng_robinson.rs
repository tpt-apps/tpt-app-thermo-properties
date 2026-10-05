//! Peng-Robinson equation of state, delegated to `tpt-thermo-eos-cubic`.
//!
//! The tpt-thermodynamics workspace now ships the machinery this engine used
//! to implement by hand: cubic-root solving, the residual (departure)
//! enthalpy/entropy terms, and critical-point-safe root selection. This
//! module adapts the engine's anchor-calibrated fluid table onto that
//! machinery behind the same three functions the rest of the engine calls:
//! [`vapor_z`], [`residual_enthalpy`], [`residual_entropy`].
//!
//! Two deliberate choices:
//! - **PR-1976 alpha.** The upstream default alpha is the SRK Soave kappa
//!   (`0.48 + 1.574ω − 0.176ω²`); Peng-Robinson's own paper uses
//!   `0.37464 + 1.54226ω − 0.26992ω²`, which is what the engine's validated
//!   numbers were produced with — supplied here as a pluggable
//!   [`AlphaFunction`].
//! - **No volume translation** on the vapor side. The saturated *liquid*
//!   density stays with the anchor-calibrated Rackett curve in
//!   `saturation.rs` (densities at the calibration anchors are exact, which
//!   beats an uncalibrated Peneloux translation for this application).
//!
//! The residual terms anchor the superheated-vapor h/s to the saturation
//! curve; the vapor-root compressibility gives vapor density.

use crate::fluids::{Fluid, FLUIDS};
use tpt_thermo_core::component::ComponentDatabase;
use tpt_thermo_core::quantities::{MolarMass, Pressure, Temperature};
use tpt_thermo_core::{EquationOfState, R};
use tpt_thermo_eos_cubic::alpha::AlphaFunction;
use tpt_thermo_eos_cubic::mixing::VdwMixing;
use tpt_thermo_eos_cubic::{PengRobinson, Phase};

/// The Peng-Robinson (1976) alpha function, `κ = 0.37464 + 1.54226ω − 0.26992ω²`.
struct PrKappaAlpha;

impl AlphaFunction for PrKappaAlpha {
    fn alpha(&self, tr: f64, omega: f64) -> f64 {
        let kappa = 0.37464 + 1.54226 * omega - 0.26992 * omega * omega;
        let m = 1.0 + kappa * (1.0 - tr.sqrt());
        m * m
    }
}

/// Presents one engine fluid as a one-component upstream database.
struct FluidDb {
    idx: usize,
}

impl ComponentDatabase for FluidDb {
    fn num_components(&self) -> usize {
        1
    }
    fn name(&self, _i: usize) -> Result<&str, tpt_thermo_core::ThermoError> {
        Ok(FLUIDS[self.idx].name)
    }
    fn critical_temperature(&self, _i: usize) -> Result<Temperature, tpt_thermo_core::ThermoError> {
        Ok(Temperature::new::<tpt_math_units::uom::si::thermodynamic_temperature::kelvin>(
            FLUIDS[self.idx].t_crit,
        ))
    }
    fn critical_pressure(&self, _i: usize) -> Result<Pressure, tpt_thermo_core::ThermoError> {
        Ok(Pressure::new::<tpt_math_units::uom::si::pressure::pascal>(
            FLUIDS[self.idx].p_crit * 1000.0,
        ))
    }
    fn acentric_factor(&self, _i: usize) -> Result<f64, tpt_thermo_core::ThermoError> {
        Ok(FLUIDS[self.idx].omega)
    }
    fn molar_mass(&self, _i: usize) -> Result<MolarMass, tpt_thermo_core::ThermoError> {
        Ok(MolarMass::new::<tpt_math_units::uom::si::molar_mass::kilogram_per_mole>(
            FLUIDS[self.idx].molar_mass / 1000.0,
        ))
    }
}

/// The upstream PR model for `fluid` (PR-1976 alpha, zero-BIP vdW mixing).
fn model(fluid: &Fluid) -> PengRobinson {
    let idx = FLUIDS
        .iter()
        .position(|f| std::ptr::eq(f, fluid))
        .expect("fluid comes from the FLUIDS table");
    PengRobinson::with_alpha_and_mixing(
        &FluidDb { idx },
        Box::new(PrKappaAlpha),
        Box::new(VdwMixing::new(1)),
    )
    .expect("one-component database always builds")
}

/// Compressibility factor of the vapor at `(t [K], p [kPa])`: the largest
/// real root of the PR cubic.
pub fn vapor_z(fluid: &Fluid, t: f64, p_kpa: f64) -> f64 {
    let m = model(fluid);
    let roots = m.engine().z_roots(
        Temperature::new::<tpt_math_units::uom::si::thermodynamic_temperature::kelvin>(t),
        Pressure::new::<tpt_math_units::uom::si::pressure::kilopascal>(p_kpa),
        &[1.0],
    );
    roots
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max)
        .max(0.0)
}

/// Residual enthalpy h^R [kJ/kg] at `(t [K], p [kPa])` (vapor root).
///
/// Delegates to the upstream residual term; the plain (untranslated) PR
/// model's `molar_enthalpy` is residual-only, so no reference bookkeeping
/// is needed — just the molar → mass conversion.
pub fn residual_enthalpy(fluid: &Fluid, t: f64, p_kpa: f64) -> f64 {
    let m = model(fluid);
    let t_q = Temperature::new::<tpt_math_units::uom::si::thermodynamic_temperature::kelvin>(t);
    let p_q = Pressure::new::<tpt_math_units::uom::si::pressure::kilopascal>(p_kpa);
    let v = m
        .solve_phase(t_q, p_q, &[1.0], Phase::Vapor)
        .expect("vapor volume solve inside the engine's validity range");
    let h_molar = m
        .molar_enthalpy(t_q, v, &[1.0])
        .expect("residual enthalpy inside the engine's validity range");
    // J/mol ÷ g/mol = kJ/kg (molar_mass is kg/kmol, numerically g/mol).
    h_molar.value / fluid.molar_mass
}

/// Residual entropy s^R [kJ/(kg·K)] at `(t [K], p [kPa])` (vapor root).
///
/// The upstream `molar_entropy` also carries an ideal-gas pressure
/// reference part (`−R·ln(p/1 bar)` for a pure fluid), which the engine's
/// saturation-anchored composition adds itself in a different datum —
/// subtract it so this returns the pure residual contribution.
pub fn residual_entropy(fluid: &Fluid, t: f64, p_kpa: f64) -> f64 {
    let m = model(fluid);
    let t_q = Temperature::new::<tpt_math_units::uom::si::thermodynamic_temperature::kelvin>(t);
    let p_q = Pressure::new::<tpt_math_units::uom::si::pressure::kilopascal>(p_kpa);
    let v = m
        .solve_phase(t_q, p_q, &[1.0], Phase::Vapor)
        .expect("vapor volume solve inside the engine's validity range");
    let s_molar = m
        .molar_entropy(t_q, v, &[1.0])
        .expect("residual entropy inside the engine's validity range");
    // s_ig (pure fluid, zero reference cp) = −R·ln(p / 1e5 Pa); add it back
    // out to leave the pure residual term.
    let s_residual_molar = s_molar.value + R * ((p_kpa * 1000.0) / 1.0e5).ln();
    s_residual_molar / fluid.molar_mass
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluids::{find, specific_gas_constant, FLUIDS};

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
