//! The built-in fluid library: common refrigerants as anchor-table data.
//!
//! Conventions:
//! - Temperatures in kelvin in the correlation code; anchor tables below are
//!   written in °C for readability and converted at evaluation time (the
//!   liquid-cp and density anchors are sheet-level 25 °C values).
//! - Pressures in kPa, specific enthalpy/entropy per kg (kJ/kg, kJ/(kg·K)).
//! - Enthalpy/entropy reference (ASHRAE convention): **h = 200 kJ/kg and
//!   s = 1.0 kJ/(kg·K) at 0 °C saturated liquid**.
//!
//! Each fluid carries:
//! - the critical constants and acentric factor every correlation shares
//!   (high-confidence NIST/ASHRAE constants, stable across sources),
//! - a short set of saturation-pressure anchors used to calibrate the
//!   Ambrose-Walton corresponding-states vapor-pressure curve (see
//!   [`crate::saturation`]),
//! - a 25 °C saturated-liquid density anchor for the Rackett-exponent shape,
//! - three liquid heat-capacity anchors (25 °C-point sheet values),
//! - three ideal-gas heat-capacity anchors (extrapolated from molar cp° data).
//!
//! R-410A is treated as **pseudo-pure** (temperature glide ≈ 0.1 K is folded
//! into the anchors; noted in the UI). R-125 is present because R-410A's
//! documentation references it; it is also exposed as a lookup fluid.

/// One working fluid's correlation data.
pub struct Fluid {
    /// Hub-facing identifier, e.g. `"r134a"`.
    pub id: &'static str,
    /// Display name, e.g. `"R-134a"`.
    pub name: &'static str,
    /// Chemical name / blend composition for the UI's fluid card.
    pub chemistry: &'static str,
    /// Molar mass [kg/kmol].
    pub molar_mass: f64,
    /// Critical temperature [K].
    pub t_crit: f64,
    /// Critical pressure [kPa].
    pub p_crit: f64,
    /// Critical density [kg/m³] (liquid-density shape anchor).
    pub rho_crit: f64,
    /// Pitzer acentric factor [-].
    pub omega: f64,
    /// Normal boiling point [K] (Watson latent-heat shape reference).
    pub t_boil: f64,
    /// Saturated-liquid density anchors `(t [°C], ρ [kg/m³])` calibrating
    /// the Rackett/Yamada-Gunn curve (see `saturation::rho_liquid`).
    pub rho_anchors: &'static [(f64, f64)],
    /// Saturation-pressure anchors `(T [°C], psat [kPa])` calibrating
    /// Ambrose-Walton. 2-4 points spanning the practical table range.
    pub psat_anchors: &'static [(f64, f64)],
    /// Saturated-liquid specific-heat anchors `(t [°C], cp [kJ/(kg·K)])`.
    pub cp_liq_anchors: &'static [(f64, f64)],
    /// Ideal-gas specific-heat anchors `(T [K], cp [kJ/(kg·K)])`.
    pub cp_gas_anchors: &'static [(f64, f64)],
}

/// Practical lookup bounds mirrored in the UI inputs.
///
/// Saturation tables run from `t_boil - 25 K` (round number, comfortably
/// above the triple point for every fluid here) to 0.98·T_crit; superheated
/// states to 200 °C and p_crit.
pub const SAT_MIN_MARGIN_K: f64 = 25.0;
pub const SAT_MAX_TR: f64 = 0.98;
pub const SUPERHEAT_MAX_C: f64 = 200.0;

/// The built-in fluids, in UI display order (most common first).
pub static FLUIDS: &[Fluid] = &[
    Fluid {
        id: "r134a",
        name: "R-134a",
        chemistry: "1,1,1,2-tetrafluoroethane · C₂H₂F₄",
        molar_mass: 102.03,
        t_crit: 374.21,
        p_crit: 4059.0,
        rho_crit: 511.9,
        omega: 0.32684,
        t_boil: 247.08,
        rho_anchors: &[(-20.0, 1434.5), (0.0, 1294.8), (25.0, 1206.0)],
        psat_anchors: &[(-26.07, 101.325), (0.0, 292.8), (40.0, 1017.0), (60.0, 1681.8)],
        cp_liq_anchors: &[(0.0, 1.336), (25.0, 1.425), (50.0, 1.541)],
        cp_gas_anchors: &[(250.0, 0.812), (300.0, 0.852), (400.0, 0.958)],
    },
    Fluid {
        id: "r32",
        name: "R-32",
        chemistry: "difluoromethane · CH₂F₂",
        molar_mass: 52.02,
        t_crit: 351.255,
        p_crit: 5782.0,
        rho_crit: 424.0,
        omega: 0.2769,
        t_boil: 221.5,
        rho_anchors: &[(-20.0, 1085.0), (0.0, 1024.0), (25.0, 960.0)],
        psat_anchors: &[(-51.6, 101.325), (-20.0, 317.3), (0.0, 813.0), (40.0, 2478.0)],
        cp_liq_anchors: &[(0.0, 1.756), (25.0, 1.972), (50.0, 2.26)],
        cp_gas_anchors: &[(250.0, 0.780), (300.0, 0.821), (400.0, 0.916)],
    },
    Fluid {
        id: "r410a",
        name: "R-410A",
        chemistry: "R-32/R-125 50/50 mass · pseudo-pure (glide ≈ 0.1 K)",
        molar_mass: 72.585,
        t_crit: 344.55,
        p_crit: 4901.0,
        rho_crit: 451.2,
        omega: 0.292,
        t_boil: 221.55,
        rho_anchors: &[(-20.0, 1208.0), (0.0, 1124.0), (25.0, 1058.0)],
        psat_anchors: &[(-51.4, 101.325), (-20.0, 399.6), (0.0, 798.7), (40.0, 2419.0)],
        cp_liq_anchors: &[(0.0, 1.55), (25.0, 1.68), (50.0, 1.86)],
        cp_gas_anchors: &[(250.0, 0.751), (300.0, 0.784), (400.0, 0.868)],
    },
    Fluid {
        id: "r125",
        name: "R-125",
        chemistry: "pentafluoroethane · C₂HF₅",
        molar_mass: 120.02,
        t_crit: 339.17,
        p_crit: 3617.7,
        rho_crit: 568.0,
        omega: 0.30517,
        t_boil: 224.55,
        rho_anchors: &[(-20.0, 1382.0), (0.0, 1302.0), (25.0, 1185.0)],
        psat_anchors: &[(-48.6, 101.325), (0.0, 578.2), (40.0, 1826.0)],
        cp_liq_anchors: &[(0.0, 1.33), (25.0, 1.40), (50.0, 1.5)],
        cp_gas_anchors: &[(250.0, 0.715), (300.0, 0.747), (400.0, 0.826)],
    },
    Fluid {
        id: "r22",
        name: "R-22",
        chemistry: "chlorodifluoromethane · CHClF₂ (HCFC, service phase-out)",
        molar_mass: 86.47,
        t_crit: 369.3,
        p_crit: 4988.0,
        rho_crit: 523.8,
        omega: 0.2206,
        t_boil: 232.34,
        rho_anchors: &[(-20.0, 1429.0), (0.0, 1285.2), (25.0, 1194.0)],
        psat_anchors: &[(-40.8, 101.325), (-20.0, 244.8), (0.0, 497.6), (40.0, 1533.5)],
        cp_liq_anchors: &[(0.0, 1.145), (25.0, 1.24), (50.0, 1.36)],
        cp_gas_anchors: &[(250.0, 0.605), (300.0, 0.648), (400.0, 0.75)],
    },
    Fluid {
        id: "r290",
        name: "R-290",
        chemistry: "propane · C₃H₈ (flammable, A3)",
        molar_mass: 44.096,
        t_crit: 369.83,
        p_crit: 4248.0,
        rho_crit: 220.5,
        omega: 0.15229,
        t_boil: 231.04,
        rho_anchors: &[(-20.0, 560.4), (0.0, 528.9), (25.0, 493.0)],
        psat_anchors: &[(-42.1, 101.325), (0.0, 474.7), (20.0, 836.3), (40.0, 1371.0)],
        cp_liq_anchors: &[(0.0, 2.32), (25.0, 2.54), (50.0, 2.82)],
        cp_gas_anchors: &[(250.0, 1.549), (300.0, 1.679), (400.0, 1.905)],
    },
    Fluid {
        id: "r600a",
        name: "R-600a",
        chemistry: "isobutane · C₄H₁₀ (flammable, A3)",
        molar_mass: 58.12,
        t_crit: 407.81,
        p_crit: 3629.0,
        rho_crit: 225.5,
        omega: 0.18353,
        t_boil: 261.4,
        rho_anchors: &[(-20.0, 604.0), (0.0, 582.0), (25.0, 551.0)],
        psat_anchors: &[(-11.75, 101.325), (0.0, 158.3), (20.0, 297.6), (40.0, 531.6)],
        cp_liq_anchors: &[(0.0, 2.2), (25.0, 2.37), (50.0, 2.57)],
        cp_gas_anchors: &[(250.0, 1.507), (300.0, 1.663), (400.0, 1.965)],
    },
    Fluid {
        id: "r717",
        name: "R-717",
        chemistry: "ammonia · NH₃ (toxic, B2L)",
        molar_mass: 17.031,
        t_crit: 405.4,
        p_crit: 11333.0,
        rho_crit: 225.0,
        omega: 0.2526,
        t_boil: 239.82,
        rho_anchors: &[(-20.0, 665.0), (0.0, 638.0), (25.0, 603.0)],
        psat_anchors: &[(-33.33, 101.325), (-20.0, 190.2), (0.0, 429.4), (20.0, 857.5), (40.0, 1554.9)],
        cp_liq_anchors: &[(0.0, 4.61), (25.0, 4.8), (50.0, 5.05)],
        cp_gas_anchors: &[(250.0, 2.024), (300.0, 2.091), (400.0, 2.29)],
    },
    Fluid {
        id: "r1234yf",
        name: "R-1234yf",
        chemistry: "2,3,3,3-tetrafluoropropene · C₃H₂F₄ (mildly flammable, A2L)",
        molar_mass: 114.04,
        t_crit: 367.85,
        p_crit: 3382.3,
        rho_crit: 492.0,
        omega: 0.2762,
        t_boil: 243.65,
        rho_anchors: &[(-20.0, 1346.0), (0.0, 1241.0), (25.0, 1087.0)],
        psat_anchors: &[(-29.5, 101.325), (-20.0, 147.5), (0.0, 315.4), (40.0, 1017.6)],
        cp_liq_anchors: &[(0.0, 1.23), (25.0, 1.31), (50.0, 1.42)],
        cp_gas_anchors: &[(250.0, 0.955), (300.0, 1.02), (400.0, 1.16)],
    },
];

/// Universal gas constant [J/(kmol·K)].
pub const R_UNIVERSAL: f64 = 8314.462618;

/// The fluid's specific gas constant [kJ/(kg·K)].
pub fn specific_gas_constant(fluid: &Fluid) -> f64 {
    R_UNIVERSAL / fluid.molar_mass / 1000.0
}

/// Looks a fluid up by hub id (`"r134a"`); case-insensitive, tolerant of a
/// leading "R-" being written as "r134a"/"R-134a"/"134a".
pub fn find(id: &str) -> Option<&'static Fluid> {
    let normalized = id.trim().to_ascii_lowercase().replace('-', "");
    let normalized = normalized.strip_prefix('r').unwrap_or(&normalized);
    FLUIDS.iter().find(|f| f.id.strip_prefix('r') == Some(normalized))
}

/// Practical saturation range for a fluid: `(t_min [K], t_max [K])`.
pub fn saturation_range(fluid: &Fluid) -> (f64, f64) {
    (
        fluid.t_boil - SAT_MIN_MARGIN_K,
        fluid.t_crit * SAT_MAX_TR,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_matches_common_spellings() {
        for f in FLUIDS {
            let with_dash = format!("R-{}", &f.id[1..]);
            assert!(find(&with_dash).is_some(), "lookup failed for {with_dash}");
            assert!(find(f.id).is_some(), "lookup failed for {}", f.id);
            let bare = &f.id[1..];
            assert!(find(bare).is_some(), "lookup failed for {bare}");
        }
        assert!(find("r9999").is_none());
        assert!(find("water").is_none());
    }

    #[test]
    fn psat_anchor_at_normal_boiling_is_atmospheric() {
        // Every fluid's first psat anchor is its normal boiling point.
        for f in FLUIDS {
            let (t_anchor, p_anchor) = f.psat_anchors[0];
            assert!((p_anchor - 101.325).abs() < 0.5, "{} first anchor not atmospheric", f.name);
            assert!(
                (t_anchor - (f.t_boil - 273.15)).abs() < 0.6,
                "{} first anchor {:.2} °C deviates from t_boil {:.2} °C",
                f.name,
                t_anchor,
                f.t_boil - 273.15
            );
        }
    }

    #[test]
    fn anchors_are_strictly_increasing() {
        for f in FLUIDS {
            assert!(f.psat_anchors.windows(2).all(|w| w[0].0 < w[1].0), "{} psat", f.name);
            assert!(f.cp_liq_anchors.windows(2).all(|w| w[0].0 < w[1].0), "{} cp_liq", f.name);
            assert!(f.cp_gas_anchors.windows(2).all(|w| w[0].0 < w[1].0), "{} cp_gas", f.name);
        }
    }
}
