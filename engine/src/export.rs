//! (pro) Calculation-sheet rendering: the exportable design documentation.
//!
//! Two formats:
//! - a **Markdown calculation sheet** — self-contained documentation with the
//!   inputs, the property/cycle results, the engine's stated accuracy bounds,
//!   and a timestamp; reads directly in Word/OneNote/GitHub and drops into
//!   project folders;
//! - **CSV** state-point tables — for spreadsheets and further calculations.

use crate::cycle::{RankineResult, RefrigerationResult, CyclePoint};
use crate::saturation;
use crate::fluids::Fluid;
use crate::state::State;

/// Accuracy statement carried on every exported sheet — the same bounds the
/// validation suite establishes (see `tests` in this crate and README).
pub const ACCURACY_NOTE: &str = concat!(
    "Accuracy (validated against published reference tables): saturation pressure within ~1.5% ",
    "of the calibration anchors; condensing/evaporating temperatures within ~0.3 K; saturated ",
    "liquid density ~2%; latent heat ~2-3% (ammonia ~3%); vapor density ~3-6%. Intended for ",
    "preliminary design and cross-checking — verify final selections against a full EOS ",
    "package (REFPROP/NIST) and manufacturer data before purchase or construction."
);

fn fmt(v: f64, digits: usize) -> String {
    format!("{v:.digits$}")
}

fn point_row(point: &CyclePoint) -> Vec<String> {
    let s = &point.state;
    vec![
        point.label.clone(),
        fmt(s.t_c, 1),
        fmt(s.p_kpa, 1),
        fmt(s.h, 2),
        fmt(s.s, 4),
        fmt(s.rho, 2),
        match s.region {
            crate::state::Region::SubcooledLiquid => "subcooled".into(),
            crate::state::Region::Saturation => format!("two-phase x={:.3}", s.quality),
            crate::state::Region::SuperheatedVapor => "superheated".into(),
        },
    ]
}

/// Markdown calculation sheet for a solved refrigeration cycle.
pub fn refrigeration_sheet(
    fluid: &Fluid,
    input: &crate::cycle::RefrigerationInput,
    result: &RefrigerationResult,
    title: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {title}\n\n"));
    out.push_str(&format!(
        "*TPT Thermo Properties · vapor-compression refrigeration cycle · generated {timestamp}*\n\n",
        timestamp = timestamp_utc()
    ));
    out.push_str("## Inputs\n\n");
    out.push_str(&format!(
        "| Parameter | Value |\n|---|---|\n"
    ));
    out.push_str(&format!(
        "| Working fluid | {} ({}) |\n",
        fluid.name, fluid.chemistry
    ));
    out.push_str(&format!("| Evaporating temperature | {} °C |\n", fmt(input.t_evap_c, 1)));
    out.push_str(&format!("| Condensing temperature | {} °C |\n", fmt(input.t_cond_c, 1)));
    out.push_str(&format!("| Suction superheat | {} K |\n", fmt(input.superheat_k, 1)));
    out.push_str(&format!("| Liquid subcooling | {} K |\n", fmt(input.subcool_k, 1)));
    out.push_str(&format!(
        "| Isentropic efficiency | {} |\n",
        fmt(input.eta_isentropic * 100.0, 1)
    ));
    out.push_str(&format!("| Evaporator capacity | {} kW |\n\n", fmt(input.capacity_kw, 2)));

    out.push_str("## Performance\n\n");
    out.push_str("| Quantity | Value |\n|---|---|\n");
    out.push_str(&format!("| **COP (cooling)** | **{}** |\n", fmt(result.cop, 3)));
    out.push_str(&format!("| COP (heating, condenser) | {} |\n", fmt(result.cop_heating, 3)));
    out.push_str(&format!("| Carnot COP | {} |\n", fmt(result.carnot_cop, 3)));
    out.push_str(&format!(
        "| Fraction of Carnot | {} |\n",
        fmt(result.carnot_fraction * 100.0, 1)
    ));
    out.push_str(&format!("| Specific evaporator duty q = h1−h4 | {} kJ/kg |\n", fmt(result.q_evap_kjkg, 2)));
    out.push_str(&format!("| Specific compressor work w = h2−h1 | {} kJ/kg |\n", fmt(result.w_comp_kjkg, 2)));
    out.push_str(&format!("| Condenser rejection q_cond = h2−h3 | {} kJ/kg |\n", fmt(result.q_cond_kjkg, 2)));
    out.push_str(&format!("| Refrigerant mass flow | {} kg/s |\n", fmt(result.mass_flow_kgs, 4)));
    out.push_str(&format!(
        "| Discharge temperature | {} °C (isentropic: {} °C) |\n",
        fmt(result.discharge_t_c, 1),
        fmt(result.discharge_t_isentropic_c, 1)
    ));
    out.push_str(&format!(
        "| Suction specific volume | {} m³/kg |\n\n",
        fmt(result.suction_v_m3kg, 5)
    ));

    out.push_str(&state_table_markdown(&result.points));

    if !result.remarks.is_empty() {
        out.push_str("## Remarks\n\n");
        for r in &result.remarks {
            out.push_str(&format!("- {r}\n"));
        }
        out.push('\n');
    }
    out.push_str(ACCURACY_NOTE);
    out.push('\n');
    out
}

/// Markdown calculation sheet for a solved Rankine cycle.
pub fn rankine_sheet(
    input: &crate::cycle::RankineInput,
    result: &RankineResult,
    title: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {title}\n\n"));
    out.push_str(&format!(
        "*TPT Thermo Properties · Rankine cycle (IAPWS-IF97 water) · generated {timestamp}*\n\n",
        timestamp = timestamp_utc()
    ));
    out.push_str("## Inputs\n\n| Parameter | Value |\n|---|---|\n");
    out.push_str(&format!("| Boiler pressure | {} MPa |\n", fmt(input.p_boiler_mpa, 2)));
    out.push_str(&format!("| Turbine inlet temperature | {} °C |\n", fmt(input.t_superheat_c, 1)));
    out.push_str(&format!("| Condenser pressure | {} kPa |\n", fmt(input.p_cond_kpa, 1)));
    out.push_str(&format!(
        "| Turbine / pump isentropic efficiency | {}% / {}% |\n\n",
        fmt(input.eta_turbine * 100.0, 1),
        fmt(input.eta_pump * 100.0, 1)
    ));

    out.push_str("## Performance\n\n| Quantity | Value |\n|---|---|\n");
    out.push_str(&format!("| **Thermal efficiency** | **{}** |\n", fmt(result.eta_thermal * 100.0, 2)));
    out.push_str(&format!("| Carnot efficiency | {} |\n", fmt(result.carnot_eta * 100.0, 2)));
    out.push_str(&format!("| Turbine work w_t = h1−h2 | {} kJ/kg |\n", fmt(result.w_turbine_kjkg, 2)));
    out.push_str(&format!("| Pump work w_p = h4−h3 | {} kJ/kg |\n", fmt(result.w_pump_kjkg, 3)));
    out.push_str(&format!("| Heat input q_in = h1−h4 | {} kJ/kg |\n", fmt(result.q_in_kjkg, 2)));
    out.push_str(&format!("| Back-work ratio | {} |\n", fmt(result.back_work_ratio, 4)));
    out.push_str(&format!(
        "| Turbine exhaust quality | {} |\n",
        if result.exhaust_quality < 1.0 { fmt(result.exhaust_quality, 3) } else { "superheated".into() }
    ));
    out.push_str(&format!(
        "| Steam rate | {} kg/kWh |\n\n",
        fmt(3600.0 / result.w_turbine_kjkg, 3)
    ));

    out.push_str(&state_table_markdown(&result.points));

    if !result.remarks.is_empty() {
        out.push_str("## Remarks\n\n");
        for r in &result.remarks {
            out.push_str(&format!("- {r}\n"));
        }
        out.push('\n');
    }
    out.push_str(ACCURACY_NOTE);
    out.push_str("\n\nWater/steam properties: IAPWS-IF97 (Regions 1, 2, 4) via tpt-eng-props-water.\n");
    out
}

/// Cycle state points as a Markdown table.
pub fn state_table_markdown(points: &[CyclePoint]) -> String {
    let mut out = String::from(
        "| Station | T [°C] | p [kPa] | h [kJ/kg] | s [kJ/(kg·K)] | ρ [kg/m³] | Region |\n|---|---|---|---|---|---|---|\n",
    );
    for p in points {
        let row = point_row(p);
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            row[0], row[1], row[2], row[3], row[4], row[5], row[6]
        ));
    }
    out.push('\n');
    out
}

/// Cycle state points as CSV (RFC-4180-escaped).
pub fn state_table_csv(points: &[CyclePoint]) -> String {
    let mut out = String::from("station,t_c,p_kpa,h_kJ_kg,s_kJ_kgK,rho_kg_m3,quality,region\r\n");
    for p in points {
        let s = &p.state;
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{}\r\n",
            csv_escape(&p.label),
            fmt(s.t_c, 2),
            fmt(s.p_kpa, 2),
            fmt(s.h, 3),
            fmt(s.s, 5),
            fmt(s.rho, 3),
            fmt(s.quality, 4),
            match s.region {
                crate::state::Region::SubcooledLiquid => "subcooled",
                crate::state::Region::Saturation => "two-phase",
                crate::state::Region::SuperheatedVapor => "superheated",
            }
        ));
    }
    out
}

/// A saturation table from `t_start_c` to `t_end_c` as CSV — the free-tier
/// lookup can also export single-fluid saturation tables under pro.
pub fn saturation_table_csv(fluid: &Fluid, t_start_c: f64, t_end_c: f64, step_k: f64) -> String {
    let mut out = String::from(
        "t_c,psat_kPa,rho_liq_kg_m3,rho_vap_kg_m3,hf_kJ_kg,hg_kJ_kg,hfg_kJ_kg,sf_kJ_kgK,sg_kJ_kgK\r\n",
    );
    let mut n = ((t_end_c - t_start_c) / step_k).round() as i64;
    n = n.clamp(0, 400);
    for k in 0..=n {
        let t_c = t_start_c + step_k * k as f64;
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\r\n",
            fmt(t_c, 1),
            fmt(saturation::psat(fluid, t_c), 2),
            fmt(saturation::rho_liquid(fluid, t_c), 2),
            fmt(saturation::rho_vapor(fluid, t_c), 4),
            fmt(saturation::hf(fluid, t_c), 2),
            fmt(saturation::hg(fluid, t_c), 2),
            fmt(saturation::hfg(fluid, t_c), 2),
            fmt(saturation::sf(fluid, t_c), 4),
            fmt(saturation::sg(fluid, t_c), 4),
        ));
    }
    out
}

fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// RFC 3339-ish UTC timestamp without pulling a date library into the wasm
/// payload: seconds since the Unix epoch are unambiguous in a sheet's
/// generation note. (Desktop/web shells format it client-side if desired.)
fn timestamp_utc() -> String {
    // web_time gives a monotonic clock, not wall time — export the engine
    // build identifier instead of a fake timestamp when no wall clock is
    // available (wasm32). On native hosts std::time is used.
    #[cfg(target_arch = "wasm32")]
    {
        "wasm session".to_string()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!("unix epoch +{secs}s")
    }
}

/// Convenience: formats a free-tier single-point lookup row for the sheet.
pub fn lookup_sheet(fluid: &Fluid, state: &State, title: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {title}\n\n"));
    out.push_str(&format!(
        "*TPT Thermo Properties · single-point property lookup · {fluid_name}*\n\n",
        fluid_name = fluid.name
    ));
    out.push_str(&state_table_markdown(&[CyclePoint {
        label: "lookup point".into(),
        state: *state,
    }]));
    out.push_str(ACCURACY_NOTE);
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_header_and_crlf() {
        let csv = state_table_csv(&[]);
        assert!(csv.starts_with("station,t_c,p_kpa,h_kJ_kg,s_kJ_kgK,rho_kg_m3,quality,region\r\n"));
        assert!(csv.ends_with("\r\n"));
    }

    #[test]
    fn saturation_csv_shapes() {
        let f = &crate::fluids::FLUIDS[0];
        let csv = saturation_table_csv(f, 0.0, 40.0, 10.0);
        let lines: Vec<&str> = csv.trim().lines().collect();
        assert_eq!(lines.len(), 6, "header + 5 rows");
        assert!(lines[1].starts_with("0.0,292"));
    }
}
