//! (pro) Thermodynamic cycle analysis: vapor-compression refrigeration over
//! the built-in refrigerant library, and Rankine over IAPWS-IF97 water.
//!
//! The refrigeration side is correlation-based (always compiles, host-testable
//! in the default test suite); the app's own `pro` feature decides whether
//! the UI exposes it. The Rankine side depends on `tpt-eng-props-water` and
//! is compiled only under the engine's `pro` feature.

use crate::fluids::{find, Fluid};
use crate::saturation;
use crate::state::{superheated_from_entropy, superheated_state, two_phase_state, State};

/// One labeled station of a solved cycle.
#[derive(Debug, Clone)]
pub struct CyclePoint {
    /// Station name, e.g. `"1 · evaporator outlet"`.
    pub label: String,
    pub state: State,
}

/// Vapor-compression refrigeration cycle definition.
#[derive(Debug, Clone)]
pub struct RefrigerationInput {
    pub fluid_id: String,
    /// Evaporating (saturation) temperature [°C].
    pub t_evap_c: f64,
    /// Condensing (saturation) temperature [°C].
    pub t_cond_c: f64,
    /// Suction superheat [K] (0 = saturated vapor at 1).
    pub superheat_k: f64,
    /// Liquid subcooling [K] (0 = saturated liquid at 3).
    pub subcool_k: f64,
    /// Compressor isentropic efficiency [0.05–1].
    pub eta_isentropic: f64,
    /// Evaporator duty [kW].
    pub capacity_kw: f64,
}

#[derive(Debug, Clone)]
pub struct RefrigerationResult {
    pub points: Vec<CyclePoint>,
    /// Evaporator specific duty [kJ/kg] (h1 − h4).
    pub q_evap_kjkg: f64,
    /// Compressor specific work [kJ/kg] (h2 − h1).
    pub w_comp_kjkg: f64,
    /// Condenser specific rejection [kJ/kg] (h2 − h3).
    pub q_cond_kjkg: f64,
    /// Coefficient of performance (cooling).
    pub cop: f64,
    /// Heating COP at the condenser (heat-pump duty).
    pub cop_heating: f64,
    /// Carnot COP between the two saturation temperatures.
    pub carnot_cop: f64,
    /// Fraction of Carnot realized.
    pub carnot_fraction: f64,
    /// Refrigerant mass flow [kg/s] for the requested capacity.
    pub mass_flow_kgs: f64,
    /// Compressor discharge temperature [°C].
    pub discharge_t_c: f64,
    /// Isentropic (ideal) discharge temperature [°C], for comparison.
    pub discharge_t_isentropic_c: f64,
    /// Suction-line specific volume at 1 [m³/kg] (displacement sizing).
    pub suction_v_m3kg: f64,
    pub solve_ms: f64,
    /// Non-fatal remarks (wet suction, near-critical pressures, clamps).
    pub remarks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CycleError(pub String);

impl std::fmt::Display for CycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Solves the vapor-compression cycle
/// `4 → (evaporator) → 1 → (compressor) → 2 → (condenser) → 3 → (valve) → 4`.
pub fn refrigeration_cycle(input: &RefrigerationInput) -> Result<RefrigerationResult, CycleError> {
    let started = web_time::Instant::now();
    let fluid: &Fluid = find(&input.fluid_id)
        .ok_or_else(|| CycleError(format!("unknown fluid '{}'", input.fluid_id)))?;

    let eta = input.eta_isentropic;
    if !(0.05..=1.0).contains(&eta) {
        return Err(CycleError(
            "Isentropic efficiency must be between 0.05 and 1.0 (1.0 = ideal).".into(),
        ));
    }
    if !(0.1..=5000.0).contains(&input.capacity_kw) || !input.capacity_kw.is_finite() {
        return Err(CycleError("Capacity must be between 0.1 and 5000 kW.".into()));
    }
    if !input.t_evap_c.is_finite()
        || !input.t_cond_c.is_finite()
        || input.t_cond_c <= input.t_evap_c
    {
        return Err(CycleError(
            "Condensing temperature must be above the evaporating temperature.".into(),
        ));
    }
    if !(0.0..=60.0).contains(&input.superheat_k) || !(0.0..=60.0).contains(&input.subcool_k) {
        return Err(CycleError("Superheat and subcooling must be between 0 and 60 K.".into()));
    }

    let (t_lo_k, t_hi_k) = crate::fluids::saturation_range(fluid);
    let t_lo_c = t_lo_k - 273.15;
    let t_hi_c = t_hi_k - 273.15;
    let mut remarks = Vec::new();
    let t_evap = input.t_evap_c.clamp(t_lo_c, t_hi_c);
    if t_evap != input.t_evap_c {
        remarks.push(format!(
            "Evaporating temperature clamped into the fluid's valid range ({t_lo_c:.0} to {t_hi_c:.0} °C)."
        ));
    }
    let t_cond = input.t_cond_c.clamp(t_evap + 1.0, t_hi_c);
    if t_cond != input.t_cond_c {
        remarks.push(format!(
            "Condensing temperature clamped into the fluid's valid range ({:.1} to {t_hi_c:.0} °C).",
            t_evap + 1.0
        ));
    }

    let p_evap = saturation::psat(fluid, t_evap);
    let p_cond = saturation::psat(fluid, t_cond);
    if !(p_evap.is_finite() && p_cond.is_finite()) {
        return Err(CycleError("Saturation pressures out of the correlation range.".into()));
    }

    // 1 — evaporator outlet (with superheat).
    let state1 = if input.superheat_k > 0.01 {
        superheated_state(fluid, t_evap + input.superheat_k, p_evap)
    } else {
        let mut s = two_phase_state(fluid, t_evap, 1.0);
        s.region = crate::state::Region::SuperheatedVapor;
        s
    };

    // 2s — isentropic compression to p_cond.
    let state2s = superheated_from_entropy(fluid, p_cond, state1.s).map_err(|_| {
        CycleError(
            "isentropic compression lands at or below the saturation line — check the input temperatures".into(),
        )
    })?;
    // 2 — actual discharge: efficiency lands on an enthalpy, so invert (p, h).
    let h2 = state1.h + (state2s.h - state1.h) / eta;
    let state2 = superheated_from_enthalpy(fluid, p_cond, h2)?;

    // 3 — condenser outlet (subcooled liquid), 4 — valve outlet (h = h3).
    let t3_c = t_cond - input.subcool_k;
    let hf3 = saturation::hf(fluid, t3_c);
    let rho_f3 = saturation::rho_liquid(fluid, t3_c);
    let h3 = hf3 + (p_cond - saturation::psat(fluid, t3_c)).max(0.0) / rho_f3;
    let state3 = State {
        t_c: t3_c,
        p_kpa: p_cond,
        h: h3,
        s: saturation::sf(fluid, t3_c),
        rho: rho_f3,
        region: crate::state::Region::SubcooledLiquid,
        quality: 0.0,
    };
    let x4 = crate::state::quality_from_enthalpy(fluid, t_evap, h3);
    let state4 = two_phase_state(fluid, t_evap, x4);

    let q_evap = state1.h - state4.h;
    let w_comp = h2 - state1.h;
    let q_cond = h2 - h3;
    let cop = q_evap / w_comp;
    let cop_heating = q_cond / w_comp;
    let carnot = (t_evap + 273.15) / (t_cond - t_evap);
    let mass_flow = input.capacity_kw / q_evap;

    if input.superheat_k < 0.01 {
        remarks.push(
            "Suction is saturated vapor (0 K superheat): real compressors need 3-10 K to avoid liquid slugging.".into(),
        );
    }
    if x4 > 0.95 {
        remarks.push("Valve outlet is nearly pure vapor — check the subcooling/temperature lift.".into());
    }
    if t_cond > t_hi_c - 5.0 || t_evap > t_hi_c - 10.0 {
        remarks.push(
            "Cycle pressures are close to the fluid's critical point; correlation accuracy degrades there.".into(),
        );
    }

    Ok(RefrigerationResult {
        points: vec![
            CyclePoint { label: "1 · evaporator outlet".into(), state: state1 },
            CyclePoint { label: "2s · isentropic discharge".into(), state: state2s },
            CyclePoint { label: "2 · actual discharge".into(), state: state2 },
            CyclePoint { label: "3 · condenser outlet".into(), state: state3 },
            CyclePoint { label: "4 · valve outlet".into(), state: state4 },
        ],
        q_evap_kjkg: q_evap,
        w_comp_kjkg: w_comp,
        q_cond_kjkg: q_cond,
        cop,
        cop_heating,
        carnot_cop: carnot,
        carnot_fraction: cop / carnot,
        mass_flow_kgs: mass_flow,
        discharge_t_c: state2.t_c,
        discharge_t_isentropic_c: state2s.t_c,
        suction_v_m3kg: 1.0 / state1.rho,
        solve_ms: started.elapsed().as_secs_f64() * 1000.0,
        remarks,
    })
}

/// Superheated state at `(p, h)` — inverts the discharge enthalpy (needed
/// because isentropic efficiency lands on an enthalpy, not an entropy).
fn superheated_from_enthalpy(fluid: &Fluid, p_kpa: f64, h: f64) -> Result<State, CycleError> {
    let t_sat_c = saturation::tsat(fluid, p_kpa);
    let h_at = |t_c: f64| superheated_state(fluid, t_c, p_kpa).h;
    let mut lo = t_sat_c;
    let mut hi = t_sat_c + 400.0;
    if h < h_at(lo) {
        // Below saturated-vapor enthalpy (efficiency below ~0.05 territory):
        // report the saturated state rather than failing.
        return Ok(two_phase_state(fluid, t_sat_c, 1.0));
    }
    if h > h_at(hi) {
        return Err(CycleError("discharge enthalpy above the correlation range".into()));
    }
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if h_at(mid) < h {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(superheated_state(fluid, 0.5 * (lo + hi), p_kpa))
}

/// Rankine cycle definition (IF97 water side).
#[cfg(feature = "pro")]
#[derive(Debug, Clone)]
pub struct RankineInput {
    /// Boiler pressure [MPa].
    pub p_boiler_mpa: f64,
    /// Turbine inlet temperature [°C] (superheat over Tsat(p_boiler)).
    pub t_superheat_c: f64,
    /// Condenser pressure [kPa].
    pub p_cond_kpa: f64,
    /// Turbine isentropic efficiency [0.05–1].
    pub eta_turbine: f64,
    /// Pump isentropic efficiency [0.05–1].
    pub eta_pump: f64,
}

#[cfg(feature = "pro")]
#[derive(Debug, Clone)]
pub struct RankineResult {
    pub points: Vec<CyclePoint>,
    /// Turbine specific work [kJ/kg] (h1 − h2).
    pub w_turbine_kjkg: f64,
    /// Pump specific work [kJ/kg] (h4 − h3).
    pub w_pump_kjkg: f64,
    /// Boiler specific heat input [kJ/kg] (h1 − h4).
    pub q_in_kjkg: f64,
    /// Thermal efficiency (w_net / q_in).
    pub eta_thermal: f64,
    /// Back-work ratio (w_pump / w_turbine).
    pub back_work_ratio: f64,
    /// Turbine exhaust quality (moisture check; 1.0 if superheated).
    pub exhaust_quality: f64,
    /// Carnot efficiency between t_superheat and Tsat(p_cond).
    pub carnot_eta: f64,
    pub solve_ms: f64,
    pub remarks: Vec<String>,
}

#[cfg(feature = "pro")]
/// Solves the Rankine cycle `3 → (pump) → 4 → (boiler) → 1 → (turbine) → 2`.
pub fn rankine_cycle(input: &RankineInput) -> Result<RankineResult, CycleError> {
    use tpt_eng_props_water as water;
    use tpt_math_units::uom::si::f64::*;
    use tpt_math_units::uom::si::{
        pressure::{kilopascal, megapascal},
        specific_heat_capacity::kilojoule_per_kilogram_kelvin,
        specific_volume::cubic_meter_per_kilogram,
        thermodynamic_temperature::kelvin,
    };

    let started = web_time::Instant::now();
    let mut remarks = Vec::new();

    if !(0.1..=16.5).contains(&input.p_boiler_mpa) {
        return Err(CycleError(
            "Boiler pressure must be between 0.1 and 16.5 MPa (IF97 Region 3 is out of scope).".into(),
        ));
    }
    if !(0.5..=200.0).contains(&input.p_cond_kpa) {
        return Err(CycleError("Condenser pressure must be between 0.5 and 200 kPa.".into()));
    }
    if !(0.05..=1.0).contains(&input.eta_turbine) || !(0.05..=1.0).contains(&input.eta_pump) {
        return Err(CycleError("Efficiencies must be between 0.05 and 1.0.".into()));
    }
    let p_boiler = Pressure::new::<megapascal>(input.p_boiler_mpa);
    let p_cond = Pressure::new::<kilopascal>(input.p_cond_kpa);

    let t_sat_boiler_c = water::saturation_temperature(p_boiler).get::<kelvin>() - 273.15;
    if input.t_superheat_c < t_sat_boiler_c + 1.0 {
        return Err(CycleError(format!(
            "Turbine inlet temperature must be at least ~{:.1} °C (saturation at {} MPa); this edition takes a superheated inlet only.",
            t_sat_boiler_c + 1.0,
            input.p_boiler_mpa
        )));
    }
    if input.t_superheat_c > 1300.0 {
        return Err(CycleError(
            "Turbine inlet temperature above IF97 Region 2 validity (1300 °C).".into(),
        ));
    }

    let s_of = |ws: &water::WaterState| ws.entropy.get::<kilojoule_per_kilogram_kelvin>();
    let rho_of =
        |ws: &water::WaterState| ws.density.get::<tpt_math_units::uom::si::mass_density::kilogram_per_cubic_meter>();

    // 1 — turbine inlet (Region 2).
    let st1 = water::state(
        ThermodynamicTemperature::new::<kelvin>(input.t_superheat_c + 273.15),
        p_boiler,
    )
    .map_err(|e| CycleError(format!("turbine inlet: {e:?}")))?;
    let h1 = st1.enthalpy;
    let s1 = s_of(&st1);

    // Saturation properties at the condenser via near-line states.
    let t_sat_cond = water::saturation_temperature(p_cond).get::<kelvin>() - 273.15;
    let sat_liq = water::state(
        ThermodynamicTemperature::new::<kelvin>(t_sat_cond + 273.15),
        Pressure::new::<kilopascal>(input.p_cond_kpa * 1.000001),
    )
    .map_err(|e| CycleError(format!("condensate state: {e:?}")))?;
    let sat_vap = water::state(
        ThermodynamicTemperature::new::<kelvin>(t_sat_cond + 273.15),
        Pressure::new::<kilopascal>(input.p_cond_kpa * 0.999999),
    )
    .map_err(|e| CycleError(format!("vapor state: {e:?}")))?;
    let hf = sat_liq.enthalpy;
    let hg = sat_vap.enthalpy;
    let s_f = s_of(&sat_liq);
    let s_g = s_of(&sat_vap);
    let v_f = sat_liq.specific_volume.get::<cubic_meter_per_kilogram>();

    // 2s/2 — turbine expansion to p_cond: quality from s, or superheated.
    let s2s_is_superheated = s1 >= s_g;
    let h2s = if s2s_is_superheated {
        let t2s_k = water_superheated_at_entropy(input.p_cond_kpa, s1)?;
        water::state(ThermodynamicTemperature::new::<kelvin>(t2s_k), p_cond)
            .map_err(|e| CycleError(format!("isentropic exit: {e:?}")))?
            .enthalpy
    } else {
        let x = ((s1 - s_f) / (s_g - s_f)).clamp(0.0, 1.0);
        hf + x * (hg - hf)
    };
    let h2 = h1 - (h1 - h2s) * input.eta_turbine;
    let x2 = if s2s_is_superheated {
        1.0
    } else {
        ((h2 - hf) / (hg - hf)).clamp(0.0, 1.0)
    };

    // 4 — pump to p_boiler: w = v·Δp with kPa·m³ = kJ.
    let w_pump_ideal = v_f * (input.p_boiler_mpa * 1000.0 - input.p_cond_kpa);
    let w_pump = w_pump_ideal / input.eta_pump;
    let h4 = hf + w_pump;

    let w_turbine = h1 - h2;
    let q_in = h1 - h4;
    let eta = (w_turbine - w_pump) / q_in;
    let carnot = 1.0 - (t_sat_cond + 273.15) / (input.t_superheat_c + 273.15);

    // Report-point states (IF97 re-evaluations for the table).
    let st2_t = water_exit_temperature(input.p_cond_kpa, h2, t_sat_cond);
    let st2 = water::state(ThermodynamicTemperature::new::<kelvin>(st2_t + 273.15), p_cond)
        .map_err(|e| CycleError(format!("turbine exit: {e:?}")))?;
    let point = |label: &str, t_c: f64, p: f64, h: f64, s: f64, rho: f64, region: crate::state::Region, x: f64| {
        CyclePoint {
            label: label.to_string(),
            state: State { t_c, p_kpa: p, h, s, rho, region, quality: x },
        }
    };

    let points = vec![
        point(
            "1 · turbine inlet",
            input.t_superheat_c,
            input.p_boiler_mpa * 1000.0,
            h1,
            s1,
            rho_of(&st1),
            crate::state::Region::SuperheatedVapor,
            1.0,
        ),
        point(
            "2 · turbine exit",
            st2_t,
            input.p_cond_kpa,
            h2,
            s_of(&st2),
            rho_of(&st2),
            if x2 < 1.0 { crate::state::Region::Saturation } else { crate::state::Region::SuperheatedVapor },
            x2,
        ),
        point(
            "3 · condensate",
            t_sat_cond,
            input.p_cond_kpa,
            hf,
            s_f,
            1.0 / v_f,
            crate::state::Region::Saturation,
            0.0,
        ),
        point(
            "4 · pump discharge",
            t_sat_cond + w_pump / 4.2, // cp_water ≈ 4.2: isentropic-style marker
            input.p_boiler_mpa * 1000.0,
            h4,
            s_f,
            1.0 / v_f,
            crate::state::Region::SubcooledLiquid,
            0.0,
        ),
    ];

    if x2 < 0.88 {
        remarks.push(format!(
            "Turbine exhaust quality x = {x2:.3} — below the ~0.88 moisture limit most mechanical-design references set."
        ));
    }
    if s2s_is_superheated && input.eta_turbine < 1.0 {
        remarks.push(
            "Exhaust is superheated; efficiency is applied to enthalpy, so the reported exit temperature is consistent with the actual h2.".into(),
        );
    }

    Ok(RankineResult {
        points,
        w_turbine_kjkg: w_turbine,
        w_pump_kjkg: w_pump,
        q_in_kjkg: q_in,
        eta_thermal: eta,
        back_work_ratio: w_pump / w_turbine,
        exhaust_quality: x2,
        carnot_eta: carnot,
        solve_ms: started.elapsed().as_secs_f64() * 1000.0,
        remarks,
    })
}

/// IF97 water at entropy `s` [kJ/(kg·K)] and pressure `p_kpa` (superheated
/// region) — bisection on temperature over the `state()` surface; returns K.
#[cfg(feature = "pro")]
fn water_superheated_at_entropy(p_kpa: f64, s: f64) -> Result<f64, CycleError> {
    use tpt_eng_props_water as water;
    use tpt_math_units::uom::si::f64::*;
    use tpt_math_units::uom::si::{
        pressure::kilopascal, specific_heat_capacity::kilojoule_per_kilogram_kelvin,
        thermodynamic_temperature::kelvin,
    };
    let p = Pressure::new::<kilopascal>(p_kpa);
    let t_sat = water::saturation_temperature(p).get::<kelvin>();
    let s_at = |t_k: f64| -> Result<f64, CycleError> {
        let st = water
            ::state(ThermodynamicTemperature::new::<kelvin>(t_k), p)
            .map_err(|e| CycleError(format!("{e:?}")))?;
        Ok(st.entropy.get::<kilojoule_per_kilogram_kelvin>())
    };
    let mut lo = t_sat + 0.5;
    let mut hi = t_sat + 1200.0;
    if s < s_at(lo)? {
        return Err(CycleError("entropy below the saturation line".into()));
    }
    for _ in 0..70 {
        let mid = 0.5 * (lo + hi);
        if s_at(mid)? < s {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(0.5 * (lo + hi))
}

/// Temperature [°C] for the turbine-exit report point: two-phase exits sit at
/// Tsat; superheated exits are inverted from the actual exit enthalpy.
#[cfg(feature = "pro")]
fn water_exit_temperature(p_kpa: f64, h: f64, t_sat_c: f64) -> f64 {
    use tpt_eng_props_water as water;
    use tpt_math_units::uom::si::f64::*;
    use tpt_math_units::uom::si::{pressure::kilopascal, thermodynamic_temperature::kelvin};
    let p = Pressure::new::<kilopascal>(p_kpa);
    let h_sat_vap = water
        ::state(
            ThermodynamicTemperature::new::<kelvin>(t_sat_c + 273.15),
            Pressure::new::<kilopascal>(p_kpa * 0.999999),
        )
        .map(|st| st.enthalpy)
        .unwrap_or(f64::NAN);
    if h <= h_sat_vap {
        return t_sat_c;
    }
    let h_at = |t_k: f64| {
        water
            ::state(ThermodynamicTemperature::new::<kelvin>(t_k), p)
            .map(|st| st.enthalpy)
            .unwrap_or(f64::NAN)
    };
    let mut lo = t_sat_c + 273.15 + 0.5;
    let mut hi = t_sat_c + 273.15 + 1200.0;
    for _ in 0..70 {
        let mid = 0.5 * (lo + hi);
        if h_at(mid) < h {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi) - 273.15
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r134a_ideal_basecase() -> RefrigerationInput {
        RefrigerationInput {
            fluid_id: "r134a".into(),
            t_evap_c: 0.0,
            t_cond_c: 40.0,
            superheat_k: 0.0,
            subcool_k: 0.0,
            eta_isentropic: 1.0,
            capacity_kw: 10.0,
        }
    }

    #[test]
    fn r134a_textbook_basecase() {
        // Classic R-134a 0 °C / 40 °C ideal cycle: textbook COP ≈ 5.4,
        // discharge above Tcond, x4 ≈ 0.28.
        let r = refrigeration_cycle(&r134a_ideal_basecase()).unwrap();
        assert!((r.cop - 5.4).abs() < 0.5, "COP = {:.3}", r.cop);
        assert!(
            r.discharge_t_isentropic_c > 40.0 && r.discharge_t_isentropic_c < 70.0,
            "T2s = {:.1} °C",
            r.discharge_t_isentropic_c
        );
        assert!(
            r.carnot_fraction > 0.7 && r.carnot_fraction < 0.95,
            "η/Carnot = {:.3}",
            r.carnot_fraction
        );
        // Structural energy balance (exact by construction).
        assert!((r.q_evap_kjkg + r.w_comp_kjkg - r.q_cond_kjkg).abs() < 1e-9);
        // Mass flow consistency with the requested capacity.
        assert!((r.mass_flow_kgs * r.q_evap_kjkg - 10.0).abs() < 1e-9);
        // The valve is isenthalpic.
        let h3 = r.points[3].state.h;
        let h4 = r.points[4].state.h;
        assert!((h3 - h4).abs() < 1e-9);
        assert!(
            r.points[4].state.quality > 0.2 && r.points[4].state.quality < 0.4,
            "x4 = {:.3}",
            r.points[4].state.quality
        );
    }

    #[test]
    fn efficiency_lowers_cop_and_raises_discharge() {
        let mut inp = r134a_ideal_basecase();
        inp.eta_isentropic = 0.7;
        let real = refrigeration_cycle(&inp).unwrap();
        let ideal = refrigeration_cycle(&r134a_ideal_basecase()).unwrap();
        assert!(real.cop < ideal.cop * 0.85, "COP(η=0.7) = {:.3}", real.cop);
        assert!(
            real.discharge_t_c > ideal.discharge_t_isentropic_c + 5.0,
            "T2 = {:.1} vs T2s = {:.1}",
            real.discharge_t_c,
            ideal.discharge_t_isentropic_c
        );
        // Heating COP exceeds cooling COP.
        assert!(real.cop_heating > real.cop);
    }

    #[test]
    fn ammonia_high_pressure_cycle_solves() {
        // Ammonia runs at higher pressures: 0/35 °C is a common industrial pair.
        let r = refrigeration_cycle(&RefrigerationInput {
            fluid_id: "r717".into(),
            t_evap_c: 0.0,
            t_cond_c: 35.0,
            superheat_k: 5.0,
            subcool_k: 3.0,
            eta_isentropic: 0.75,
            capacity_kw: 250.0,
        })
        .unwrap();
        // Industrial NH3 plants see COPs ~4-5 at this lift.
        assert!(r.cop > 3.5 && r.cop < 6.0, "COP = {:.3}", r.cop);
        assert!(r.discharge_t_c > 60.0, "discharge = {:.1} °C", r.discharge_t_c);
    }

    #[test]
    fn subcooling_improves_cop() {
        let mut inp = r134a_ideal_basecase();
        inp.subcool_k = 5.0;
        let sub = refrigeration_cycle(&inp).unwrap();
        let base = refrigeration_cycle(&r134a_ideal_basecase()).unwrap();
        assert!(sub.cop > base.cop);
        assert!(sub.q_evap_kjkg > base.q_evap_kjkg);
    }

    #[test]
    fn bad_inputs_error() {
        let base = r134a_ideal_basecase();
        let mut reversed = base.clone();
        reversed.t_evap_c = 40.0;
        reversed.t_cond_c = 0.0;
        assert!(refrigeration_cycle(&reversed).is_err());
        let mut unknown = base;
        unknown.fluid_id = "nope".into();
        assert!(refrigeration_cycle(&unknown).is_err());
    }

    #[cfg(feature = "pro")]
    #[test]
    fn rankine_textbook_basecase() {
        // Moran-style base case: 8 MPa / 480 °C / 8 kPa, ideal components.
        // Reference values: η ≈ 0.397, x2 ≈ 0.794, BWR ≈ 0.006.
        let r = rankine_cycle(&RankineInput {
            p_boiler_mpa: 8.0,
            t_superheat_c: 480.0,
            p_cond_kpa: 8.0,
            eta_turbine: 1.0,
            eta_pump: 1.0,
        })
        .unwrap();
        assert!((r.eta_thermal - 0.397).abs() < 0.02, "η = {:.4}", r.eta_thermal);
        assert!((r.exhaust_quality - 0.794).abs() < 0.02, "x2 = {:.4}", r.exhaust_quality);
        assert!(r.back_work_ratio < 0.02, "BWR = {:.4}", r.back_work_ratio);
        assert!(r.eta_thermal < r.carnot_eta);
    }

    #[cfg(feature = "pro")]
    #[test]
    fn rankine_real_efficiencies_reduce_output() {
        let r = rankine_cycle(&RankineInput {
            p_boiler_mpa: 8.0,
            t_superheat_c: 480.0,
            p_cond_kpa: 8.0,
            eta_turbine: 0.9,
            eta_pump: 0.8,
        })
        .unwrap();
        assert!(r.eta_thermal > 0.33 && r.eta_thermal < 0.39, "η = {:.4}", r.eta_thermal);
        assert!(r.exhaust_quality > 0.8, "x2 = {:.4}", r.exhaust_quality);
    }
}
