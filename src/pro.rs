//! (pro) Pro edition UI: refrigeration/Rankine cycle cards, P-h diagram,
//! saved-calculation library, and exportable calculation sheets.

use std::rc::Rc;

use tpt_appfront_core::{ContainerBuilder, Signal, UITree};
use tpt_thermo_engine::cycle::{rankine_cycle, refrigeration_cycle, RankineInput, RankineResult, RefrigerationInput, RefrigerationResult, CycleError};
use tpt_thermo_engine::fluids::{self, Fluid};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::library;

/// Cycle solve outcomes (mirrors the free tier's `Outcome` shape).
#[derive(Clone)]
pub enum CycleOutcome {
    Idle,
    Invalid(Vec<String>),
    Solved(Rc<SolvedCycle>),
}

pub struct SolvedCycle {
    pub fluid: &'static Fluid,
    pub input: RefrigerationInput,
    pub result: RefrigerationResult,
    pub title: String,
}

#[derive(Clone)]
pub enum RankineOutcome {
    Idle,
    Invalid(Vec<String>),
    Solved(Rc<SolvedRankine>),
}

pub struct SolvedRankine {
    pub input: RankineInput,
    pub result: RankineResult,
    pub title: String,
}

/// Saved-library list state (drives the reactive list).
#[derive(Clone)]
pub enum LibraryOutcome {
    /// (reload counter, entries) — the counter forces re-render after CRUD.
    List(Rc<(u64, Vec<library::Entry>)>),
}

pub fn refresh_library(signal: &Signal<LibraryOutcome>, counter: u64) {
    signal.set(LibraryOutcome::List(Rc::new((counter, library::list()))));
}

// ---------------------------------------------------------------------------
// Sidebar cards (built inside the static shell).
// ---------------------------------------------------------------------------

pub fn sidebar_cards(sidebar: &mut ContainerBuilder<crate::app::Msg>) {
    sidebar.container(|card| {
        card.heading(3, "Refrigeration cycle");
        card.container(|row| {
            row.container(|field| {
                label(field, "Working fluid");
                field
                    .select(
                        fluids::FLUIDS.iter().map(|f| (f.id, f.name)).collect::<Vec<_>>(),
                        "r134a",
                    )
                    .class("thermo-input thermo-ref-fluid");
            })
            .class("thermo-field");
        })
        .class("thermo-row");
        card.container(|row| {
            row.container(|field| {
                label(field, "Evaporating T (\u{b0}C)");
                field.input("0").class("thermo-input thermo-ref-tevap");
            })
            .class("thermo-field");
            row.container(|field| {
                label(field, "Condensing T (\u{b0}C)");
                field.input("40").class("thermo-input thermo-ref-tcond");
            })
            .class("thermo-field");
        })
        .class("thermo-row");
        card.container(|row| {
            row.container(|field| {
                label(field, "Superheat (K)");
                field.input("5").class("thermo-input thermo-ref-sh");
            })
            .class("thermo-field");
            row.container(|field| {
                label(field, "Subcooling (K)");
                field.input("0").class("thermo-input thermo-ref-sc");
            })
            .class("thermo-field");
        })
        .class("thermo-row");
        card.container(|row| {
            row.container(|field| {
                label(field, "Isentropic eff. (0-1)");
                field.input("0.7").class("thermo-input thermo-ref-eta");
            })
            .class("thermo-field");
            row.container(|field| {
                label(field, "Capacity (kW)");
                field.input("10").class("thermo-input thermo-ref-cap");
            })
            .class("thermo-field");
        })
        .class("thermo-row");
        card.container(|actions| {
            actions
                .button("Analyze cycle")
                .class("thermo-btn thermo-primary")
                .on_click(crate::app::Msg::SolveRefCycle);
        })
        .class("thermo-actions");
        card.text("Vapor-compression cycle: evaporator → compressor (with isentropic efficiency) → condenser → expansion valve. COP, work, duty, mass flow and discharge temperature are reported with the state points.")
            .class("thermo-hint");
    })
    .class("thermo-card");

    sidebar.container(|card| {
        card.heading(3, "Rankine cycle (water/steam)");
        card.container(|row| {
            row.container(|field| {
                label(field, "Boiler pressure (MPa)");
                field.input("8").class("thermo-input thermo-rk-pb");
            })
            .class("thermo-field");
            row.container(|field| {
                label(field, "Turbine inlet T (\u{b0}C)");
                field.input("480").class("thermo-input thermo-rk-tsup");
            })
            .class("thermo-field");
        })
        .class("thermo-row");
        card.container(|row| {
            row.container(|field| {
                label(field, "Condenser pressure (kPa)");
                field.input("8").class("thermo-input thermo-rk-pc");
            })
            .class("thermo-field");
            row.container(|field| {
                label(field, "Turbine / pump eff.");
                field.input("0.9/0.8").class("thermo-input thermo-rk-eta");
            })
            .class("thermo-field");
        })
        .class("thermo-row");
        card.container(|actions| {
            actions
                .button("Analyze Rankine")
                .class("thermo-btn thermo-primary")
                .on_click(crate::app::Msg::SolveRankine);
        })
        .class("thermo-actions");
        card.text("Turbine/pump efficiencies as \u{201c}0.9/0.8\u{201d}. Steam properties from IAPWS-IF97 (Regions 1/2/4).")
            .class("thermo-hint");
    })
    .class("thermo-card");

    sidebar.container(|card| {
        card.heading(3, "Saved calculations");
        card.container(|row| {
            row.container(|field| {
                label(field, "Name for Save");
                field.input("").class("thermo-input thermo-lib-name");
            })
            .class("thermo-field");
        })
        .class("thermo-row");
        card.container(|actions| {
            actions
                .button("Save current")
                .class("thermo-btn")
                .on_click(crate::app::Msg::SaveCurrent);
            actions
                .button("Export sheet (.md)")
                .class("thermo-btn")
                .on_click(crate::app::Msg::ExportSheet);
            actions
                .button("Export points (.csv)")
                .class("thermo-btn")
                .on_click(crate::app::Msg::ExportCsv);
        })
        .class("thermo-actions");
        card.text("Save keeps the current lookup or cycle's inputs; Export writes a Markdown calculation sheet (or the state points as CSV). Saved calcs live in this browser's local storage.")
            .class("thermo-hint");
        // Reactive list placeholder.
        card.container(|_| {}).class("thermo-lib-list");
    })
    .class("thermo-card");
}

pub fn main_sections(main: &mut ContainerBuilder<crate::app::Msg>) {
    // P-h diagram card (canvas appended by mount code).
    main.container(|card| {
        card.heading(3, "P-h diagram");
        card.container(|holder| {}).class("thermo-ph-viewer");
    })
    .class("thermo-card");

    // Reactive refrigeration results.
    main.container(|el| {}).class("thermo-ref-results");
    // Reactive Rankine results.
    main.container(|el| {}).class("thermo-rankine-results");
}

// ---------------------------------------------------------------------------
// Lookup of the reactive views.
// ---------------------------------------------------------------------------

pub fn ref_results_view(outcome: Signal<CycleOutcome>) -> UITree<crate::app::Msg> {
    let outcome = outcome.get();
    let mut b = ContainerBuilder::<crate::app::Msg>::new();
    b.container(|c| {
        c.container(|summary| {
            let (headline, detail) = match &outcome {
                CycleOutcome::Idle => (
                    "Set the cycle, then press Analyze cycle.".to_string(),
                    "Try the defaults: an R-134a 0/40 °C system at 70% compressor efficiency.".to_string(),
                ),
                CycleOutcome::Invalid(issues) => (
                    format!(
                        "Check {} input issue{}:",
                        issues.len(),
                        if issues.len() == 1 { "" } else { "s" }
                    ),
                    String::new(),
                ),
                CycleOutcome::Solved(s) => {
                    let r = &s.result;
                    (
                        format!(
                            "{} cycle \u{b7} COP {} \u{b7} mass flow {} kg/s \u{b7} discharge {} °C",
                            s.fluid.name,
                            fmt(r.cop, 2),
                            fmt(r.mass_flow_kgs, 4),
                            fmt(r.discharge_t_c, 1)
                        ),
                        format!(
                            "q = {} kJ/kg \u{b7} w = {} kJ/kg \u{b7} q_cond = {} kJ/kg \u{b7} {}% of Carnot \u{b7} solved in {:.1} ms",
                            fmt(r.q_evap_kjkg, 1),
                            fmt(r.w_comp_kjkg, 1),
                            fmt(r.q_cond_kjkg, 1),
                            fmt(r.carnot_fraction * 100.0, 0),
                            r.solve_ms
                        ),
                    )
                }
            };
            crate::app::styled_text_pub(summary, headline, "thermo-headline");
            if !detail.is_empty() {
                crate::app::styled_text_pub(summary, detail, "thermo-detail");
            }
        })
        .class("thermo-summary");

        c.container(|issues| {
            if let CycleOutcome::Invalid(list) = &outcome {
                for issue in list {
                    crate::app::styled_text_pub(issues, issue.clone(), "thermo-issue");
                }
            }
        })
        .class("thermo-issues");

        if let CycleOutcome::Solved(s) = &outcome {
            let r = &s.result;
            c.data_grid(
                ["Quantity", "Value", "Detail"],
                vec![
                    row3("COP (cooling)", fmt(r.cop, 3), &format!("Carnot {} · heating COP {}", fmt(r.carnot_cop, 2), fmt(r.cop_heating, 2))),
                    row3("Specific duty q = h1−h4", &format!("{} kJ/kg", fmt(r.q_evap_kjkg, 2)), &format!("capacity {} kW", fmt(s.input.capacity_kw, 2))),
                    row3("Compressor work w = h2−h1", &format!("{} kJ/kg", fmt(r.w_comp_kjkg, 2)), &format!("ηs {}", fmt(s.input.eta_isentropic, 2))),
                    row3("Condenser rejection", &format!("{} kJ/kg", fmt(r.q_cond_kjkg, 2)), &format!("{} kW", fmt(r.mass_flow_kgs * r.q_cond_kjkg, 2))),
                    row3("Mass flow", &format!("{} kg/s", fmt(r.mass_flow_kgs, 4)), &format!("{} kg/h", fmt(r.mass_flow_kgs * 3600.0, 1))),
                    row3("Discharge temperature", &format!("{} °C", fmt(r.discharge_t_c, 1)), &format!("isentropic {} °C", fmt(r.discharge_t_isentropic_c, 1))),
                    row3("Suction specific volume", &format!("{} m³/kg", fmt(r.suction_v_m3kg, 5)), "displacement sizing"),
                    row3(
                        "Displacement",
                        &format!("{} m³/h", fmt(r.mass_flow_kgs * r.suction_v_m3kg * 3600.0, 2)),
                        "swept volume at 100% volumetric eff.",
                    ),
                ],
            );
            c.data_grid(["Station", "T °C", "p kPa", "h kJ/kg", "s kJ/(kg·K)", "ρ kg/m³"], points_rows(&r.points));

            if !r.remarks.is_empty() {
                c.container(|notes| {
                    for remark in &r.remarks {
                        crate::app::styled_text_pub(notes, remark.clone(), "thermo-detail");
                    }
                })
                .class("thermo-issues");
            }
        }
    })
    .class("thermo-results-view");
    b.into_only_child().expect("ref results build one root")
}

pub fn rankine_results_view(outcome: Signal<RankineOutcome>) -> UITree<crate::app::Msg> {
    let outcome = outcome.get();
    let mut b = ContainerBuilder::<crate::app::Msg>::new();
    b.container(|c| {
        c.container(|summary| {
            let (headline, detail) = match &outcome {
                RankineOutcome::Idle => (
                    "Set the cycle, then press Analyze Rankine.".to_string(),
                    "Defaults: 8 MPa / 480 °C / 8 kPa with 90%/80% turbine/pump efficiency.".to_string(),
                ),
                RankineOutcome::Invalid(issues) => (
                    format!(
                        "Check {} input issue{}:",
                        issues.len(),
                        if issues.len() == 1 { "" } else { "s" }
                    ),
                    String::new(),
                ),
                RankineOutcome::Solved(s) => (
                    format!(
                        "Rankine \u{b7} thermal efficiency {}% \u{b7} turbine {} kJ/kg \u{b7} pump {} kJ/kg",
                        fmt(s.result.eta_thermal * 100.0, 2),
                        fmt(s.result.w_turbine_kjkg, 1),
                        fmt(s.result.w_pump_kjkg, 2)
                    ),
                    format!(
                        "q_in = {} kJ/kg \u{b7} back-work {} \u{b7} steam rate {} kg/kWh \u{b7} {:.1} ms",
                        fmt(s.result.q_in_kjkg, 1),
                        fmt(s.result.back_work_ratio, 4),
                        fmt(3600.0 / s.result.w_turbine_kjkg, 2),
                        s.result.solve_ms
                    ),
                ),
            };
            crate::app::styled_text_pub(summary, headline, "thermo-headline");
            if !detail.is_empty() {
                crate::app::styled_text_pub(summary, detail, "thermo-detail");
            }
        })
        .class("thermo-summary");

        c.container(|issues| {
            if let RankineOutcome::Invalid(list) = &outcome {
                for issue in list {
                    crate::app::styled_text_pub(issues, issue.clone(), "thermo-issue");
                }
            }
        })
        .class("thermo-issues");

        if let RankineOutcome::Solved(s) = &outcome {
            let r = &s.result;
            c.data_grid(
                ["Quantity", "Value", "Detail"],
                vec![
                    row3("Thermal efficiency", &format!("{} %", fmt(r.eta_thermal * 100.0, 2)), &format!("Carnot {} %", fmt(r.carnot_eta * 100.0, 2))),
                    row3("Turbine work h1−h2", &format!("{} kJ/kg", fmt(r.w_turbine_kjkg, 2)), ""),
                    row3("Pump work h4−h3", &format!("{} kJ/kg", fmt(r.w_pump_kjkg, 3)), &format!("ηp {}", fmt(s.input.eta_pump, 2))),
                    row3("Heat input h1−h4", &format!("{} kJ/kg", fmt(r.q_in_kjkg, 2)), ""),
                    row3("Back-work ratio", fmt(r.back_work_ratio, 4), ""),
                    row3(
                        "Exhaust quality",
                        if r.exhaust_quality < 1.0 { fmt(r.exhaust_quality, 3) } else { "superheated".to_string() },
                        "moisture check",
                    ),
                    row3("Steam rate", &format!("{} kg/kWh", fmt(3600.0 / r.w_turbine_kjkg, 3)), ""),
                ],
            );
            c.data_grid(["Station", "T °C", "p kPa", "h kJ/kg", "s kJ/(kg·K)", "ρ kg/m³"], points_rows(&r.points));

            if !r.remarks.is_empty() {
                c.container(|notes| {
                    for remark in &r.remarks {
                        crate::app::styled_text_pub(notes, remark.clone(), "thermo-detail");
                    }
                })
                .class("thermo-issues");
            }
        }
    })
    .class("thermo-results-view");
    b.into_only_child().expect("rankine results build one root")
}

/// The saved-calcs list (reactive; buttons dispatch Load/Delete with the id).
pub fn library_view(outcome: Signal<LibraryOutcome>) -> UITree<crate::app::Msg> {
    let outcome = outcome.get();
    let mut b = ContainerBuilder::<crate::app::Msg>::new();
    let entries: Vec<library::Entry> = match &outcome {
        LibraryOutcome::List(pair) => pair.1.clone(),
    };
    b.container(|c| {
        {
            if entries.is_empty() {
                c.text("No saved calculations yet.").class("thermo-hint");
            }
            for e in entries.iter() {
                c.container(|row| {
                    row.container(|cell| {
                        cell.text(format!("{} — {}", e.name, kind_label(&e.kind)))
                            .class("thermo-label");
                    })
                    .class("thermo-field");
                    row.container(|cell| {
                        cell.button("Load")
                            .class("thermo-btn")
                            .on_click(crate::app::Msg::LoadCalc(e.id.clone()));
                        cell.button("Delete")
                            .class("thermo-btn")
                            .on_click(crate::app::Msg::DeleteCalc(e.id.clone()));
                    })
                    .class("thermo-actions");
                })
                .class("thermo-row");
            }
        }
    })
    .class("thermo-lib-view");
    b.into_only_child().expect("library view builds one root")
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "lookup" => "property lookup",
        "refcycle" => "refrigeration cycle",
        "rankine" => "Rankine cycle",
        _ => "calculation",
    }
}

fn points_rows(points: &[tpt_thermo_engine::cycle::CyclePoint]) -> Vec<Vec<String>> {
    points
        .iter()
        .map(|p| {
            vec![
                p.label.clone(),
                fmt(p.state.t_c, 1),
                fmt(p.state.p_kpa, 1),
                fmt(p.state.h, 2),
                fmt(p.state.s, 4),
                fmt(p.state.rho, 2),
            ]
        })
        .collect()
}

fn row3(a: impl Into<String>, b: impl Into<String>, c: impl Into<String>) -> Vec<String> {
    vec![a.into(), b.into(), c.into()]
}

// ---------------------------------------------------------------------------
// Actions (called from the dispatch closure).
// ---------------------------------------------------------------------------

fn num(container: &web_sys::Element, selector: &str, label: &str, issues: &mut Vec<String>) -> f64 {
    let raw = crate::app::read_field_pub(container, selector);
    match raw.trim().parse::<f64>() {
        Ok(v) if v.is_finite() => v,
        _ => {
            issues.push(format!("{label} must be a number (got \u{201c}{raw}\u{201d})."));
            f64::NAN
        }
    }
}

pub fn solve_ref_cycle(container: &web_sys::Element, outcome: &Signal<CycleOutcome>) {
    let mut issues = Vec::new();
    let fluid_id = crate::app::read_field_pub(container, ".thermo-ref-fluid");
    let t_evap = num(container, ".thermo-ref-tevap", "Evaporating temperature", &mut issues);
    let t_cond = num(container, ".thermo-ref-tcond", "Condensing temperature", &mut issues);
    let superheat = num(container, ".thermo-ref-sh", "Superheat", &mut issues);
    let subcool = num(container, ".thermo-ref-sc", "Subcooling", &mut issues);
    let eta = num(container, ".thermo-ref-eta", "Isentropic efficiency", &mut issues);
    let capacity = num(container, ".thermo-ref-cap", "Capacity", &mut issues);
    if !issues.is_empty() {
        outcome.set(CycleOutcome::Invalid(issues));
        return;
    }
    let input = RefrigerationInput {
        fluid_id,
        t_evap_c: t_evap,
        t_cond_c: t_cond,
        superheat_k: superheat,
        subcool_k: subcool,
        eta_isentropic: eta,
        capacity_kw: capacity,
    };
    match refrigeration_cycle(&input) {
        Ok(result) => {
            let fluid = fluids::find(&input.fluid_id).expect("validated by solve");
            outcome.set(CycleOutcome::Solved(Rc::new(SolvedCycle {
                fluid,
                title: format!(
                    "{} refrigeration cycle: {} °C evaporating / {} °C condensing",
                    fluid.name, input.t_evap_c, input.t_cond_c
                ),
                input,
                result,
            })));
        }
        Err(CycleError(msg)) => outcome.set(CycleOutcome::Invalid(vec![msg])),
    }
}

pub fn solve_rankine(container: &web_sys::Element, outcome: &Signal<RankineOutcome>) {
    let mut issues = Vec::new();
    let p_boiler = num(container, ".thermo-rk-pb", "Boiler pressure", &mut issues);
    let t_sup = num(container, ".thermo-rk-tsup", "Turbine inlet temperature", &mut issues);
    let p_cond = num(container, ".thermo-rk-pc", "Condenser pressure", &mut issues);
    let eta_raw = crate::app::read_field_pub(container, ".thermo-rk-eta");
    let (eta_t, eta_p) = parse_eta_pair(&eta_raw);
    if eta_t.is_none() || eta_p.is_none() {
        issues.push("Efficiencies must look like \u{201c}0.9/0.8\u{201d} (turbine/pump, 0-1).".to_string());
    }
    if !issues.is_empty() {
        outcome.set(RankineOutcome::Invalid(issues));
        return;
    }
    let input = RankineInput {
        p_boiler_mpa: p_boiler,
        t_superheat_c: t_sup,
        p_cond_kpa: p_cond,
        eta_turbine: eta_t.unwrap(),
        eta_pump: eta_p.unwrap(),
    };
    match rankine_cycle(&input) {
        Ok(result) => outcome.set(RankineOutcome::Solved(Rc::new(SolvedRankine {
            title: format!(
                "Rankine cycle: {} MPa / {} °C / {} kPa",
                input.p_boiler_mpa, input.t_superheat_c, input.p_cond_kpa
            ),
            input,
            result,
        }))),
        Err(CycleError(msg)) => outcome.set(RankineOutcome::Invalid(vec![msg])),
    }
}

fn parse_eta_pair(raw: &str) -> (Option<f64>, Option<f64>) {
    let mut parts = raw.split('/');
    let t = parts.next().and_then(|s| s.trim().parse::<f64>().ok());
    let p = match parts.next() {
        Some(s) => s.trim().parse::<f64>().ok(),
        None => t,
    };
    (t.filter(|v| (0.05..=1.0).contains(v)), p.filter(|v| (0.05..=1.0).contains(v)))
}

fn rankine_or_lookup(
    lookup_outcome: &crate::app::Outcome,
    rankine_outcome: &RankineOutcome,
) -> CurrentCalc {
    match rankine_outcome {
        RankineOutcome::Solved(s) => CurrentCalc::Rankine(s.clone()),
        _ => match lookup_outcome {
            crate::app::Outcome::Solved(solved) => CurrentCalc::Lookup(solved.clone()),
            _ => CurrentCalc::None,
        },
    }
}

fn rankine_or_ref(ref_outcome: &CycleOutcome, rankine_outcome: &RankineOutcome) -> CurrentCalc {
    match rankine_outcome {
        RankineOutcome::Solved(s) => CurrentCalc::Rankine(s.clone()),
        _ => match ref_outcome {
            CycleOutcome::Solved(s) => CurrentCalc::Ref(s.clone()),
            _ => CurrentCalc::None,
        },
    }
}

fn fallback_current(lookup_outcome: &crate::app::Outcome, ref_outcome: &CycleOutcome) -> CurrentCalc {
    match ref_outcome {
        CycleOutcome::Solved(s) => CurrentCalc::Ref(s.clone()),
        _ => match lookup_outcome {
            crate::app::Outcome::Solved(solved) => CurrentCalc::Lookup(solved.clone()),
            _ => CurrentCalc::None,
        },
    }
}

/// Snapshot of whatever is currently solved, for Save/Export.
pub enum CurrentCalc {
    None,
    Lookup(Rc<crate::app::Solved>),
    Ref(Rc<SolvedCycle>),
    Rankine(Rc<SolvedRankine>),
}

/// Captures the current UI state + latest results as something savable.
pub fn capture_current(
    container: &web_sys::Element,
    lookup_outcome: &crate::app::Outcome,
    ref_outcome: &CycleOutcome,
    rankine_outcome: &RankineOutcome,
) -> CurrentCalc {
    // The section the user solved last is the one they mean to keep; fall
    // back through the others when it holds nothing.
    match crate::app::last_solved() {
        "rankine" => match rankine_outcome {
            RankineOutcome::Solved(s) => CurrentCalc::Rankine(s.clone()),
            _ => fallback_current(lookup_outcome, ref_outcome),
        },
        "ref" => match ref_outcome {
            CycleOutcome::Solved(s) => CurrentCalc::Ref(s.clone()),
            _ => rankine_or_lookup(lookup_outcome, rankine_outcome),
        },
        _ => match lookup_outcome {
            crate::app::Outcome::Solved(solved) => CurrentCalc::Lookup(solved.clone()),
            _ => rankine_or_ref(ref_outcome, rankine_outcome),
        },
    }
}

pub fn save_current(calc: &CurrentCalc) -> Result<String, String> {
    let (name_default, kind, payload) = match calc {
        CurrentCalc::None => return Err("Nothing to save yet — run a lookup or a cycle first.".into()),
        CurrentCalc::Lookup(solved) => (
            format!("{} lookup", solved.fluid.name),
            "lookup",
            library::payload(&[
                ("fluid", solved.fluid.id),
                (
                    "mode",
                    match solved.mode {
                        crate::app::Mode::SatByTemperature => "sat-t",
                        crate::app::Mode::SatByPressure => "sat-p",
                        crate::app::Mode::Superheated => "super",
                    },
                ),
                (
                    "t",
                    &fmt(
                        solved
                            .t_sat_c
                            .unwrap_or_else(|| solved.point.as_ref().map(|st| st.t_c).unwrap_or(f64::NAN)),
                        2,
                    ),
                ),
                (
                    "p",
                    &fmt(
                        solved
                            .p_sat_kpa
                            .unwrap_or_else(|| solved.point.as_ref().map(|st| st.p_kpa).unwrap_or(f64::NAN)),
                        2,
                    ),
                ),
            ]),
        ),
        CurrentCalc::Ref(s) => (
            format!(
                "{} cycle {}-{} °C",
                s.fluid.name,
                fmt(s.input.t_evap_c, 0),
                fmt(s.input.t_cond_c, 0)
            ),
            "refcycle",
            library::payload(&[
                ("fluid", s.input.fluid_id.as_str()),
                ("tevap", &fmt(s.input.t_evap_c, 2)),
                ("tcond", &fmt(s.input.t_cond_c, 2)),
                ("sh", &fmt(s.input.superheat_k, 2)),
                ("sc", &fmt(s.input.subcool_k, 2)),
                ("eta", &fmt(s.input.eta_isentropic, 3)),
                ("cap", &fmt(s.input.capacity_kw, 2)),
            ]),
        ),
        CurrentCalc::Rankine(s) => (
            format!(
                "Rankine {} MPa / {} °C",
                fmt(s.input.p_boiler_mpa, 1),
                fmt(s.input.t_superheat_c, 0)
            ),
            "rankine",
            library::payload(&[
                ("pb", &fmt(s.input.p_boiler_mpa, 3)),
                ("tsup", &fmt(s.input.t_superheat_c, 1)),
                ("pc", &fmt(s.input.p_cond_kpa, 2)),
                ("etat", &fmt(s.input.eta_turbine, 3)),
                ("etap", &fmt(s.input.eta_pump, 3)),
            ]),
        ),
    };
    library::save(&name_default, kind, &payload).ok_or_else(|| "localStorage unavailable".to_string())
}

/// Re-runs a saved calculation: writes its inputs back into the form.
pub fn load_calc(
    entry: &library::Entry,
    container: &web_sys::Element,
    lookup_outcome: &Signal<crate::app::Outcome>,
    ref_outcome: &Signal<CycleOutcome>,
    rankine_outcome: &Signal<RankineOutcome>,
) {
    let p = |k: &str| library::payload_field(&entry.payload, k).unwrap_or_default();
    match entry.kind.as_str() {
        "lookup" => {
            crate::app::write_field_pub(container, ".thermo-fluid", &p("fluid"));
            crate::app::write_field_pub(container, ".thermo-mode", &p("mode"));
            crate::app::apply_mode_pub(container, &p("mode"));
            crate::app::write_field_pub(container, ".thermo-temp", &p("t"));
            crate::app::write_field_pub(container, ".thermo-press", &p("p"));
            crate::app::run_lookup_pub(container, lookup_outcome);
        }
        "refcycle" => {
            crate::app::write_field_pub(container, ".thermo-ref-fluid", &p("fluid"));
            crate::app::write_field_pub(container, ".thermo-ref-tevap", &p("tevap"));
            crate::app::write_field_pub(container, ".thermo-ref-tcond", &p("tcond"));
            crate::app::write_field_pub(container, ".thermo-ref-sh", &p("sh"));
            crate::app::write_field_pub(container, ".thermo-ref-sc", &p("sc"));
            crate::app::write_field_pub(container, ".thermo-ref-eta", &p("eta"));
            crate::app::write_field_pub(container, ".thermo-ref-cap", &p("cap"));
            solve_ref_cycle(container, ref_outcome);
        }
        "rankine" => {
            crate::app::write_field_pub(container, ".thermo-rk-pb", &p("pb"));
            crate::app::write_field_pub(container, ".thermo-rk-tsup", &p("tsup"));
            crate::app::write_field_pub(container, ".thermo-rk-pc", &p("pc"));
            crate::app::write_field_pub(
                container,
                ".thermo-rk-eta",
                &format!("{}/{}", p("etat"), p("etap")),
            );
            solve_rankine(container, rankine_outcome);
        }
        _ => {}
    }
}

/// The lookup's evaluated point as a printable State.
fn lookup_point(solved: &crate::app::Solved) -> Result<tpt_thermo_engine::state::State, String> {
    if let Some(st) = solved.point {
        return Ok(st);
    }
    match (solved.t_sat_c, solved.p_sat_kpa) {
        (Some(t), Some(p)) => {
            let mut st = tpt_thermo_engine::state::two_phase_state(solved.fluid, t, 1.0);
            st.p_kpa = p;
            Ok(st)
        }
        _ => Err("the lookup did not produce a point".into()),
    }
}

/// Downloads the Markdown calculation sheet for the current calc.
pub fn export_calc_sheet(calc: &CurrentCalc) -> Result<(), String> {
    match calc {
        CurrentCalc::None => Err("Nothing to export yet — run a lookup or a cycle first.".into()),
        CurrentCalc::Lookup(solved) => {
            let st = lookup_point(solved)?;
            let md = tpt_thermo_engine::export::lookup_sheet(
                solved.fluid,
                &st,
                &format!("{} property lookup", solved.fluid.name),
            );
            download(&format!("{}-lookup.md", solved.fluid.id), "text/markdown", &md);
            Ok(())
        }
        CurrentCalc::Ref(s) => {
            let md = tpt_thermo_engine::export::refrigeration_sheet(s.fluid, &s.input, &s.result, &s.title);
            download("refrigeration-cycle.md", "text/markdown", &md);
            Ok(())
        }
        CurrentCalc::Rankine(s) => {
            let md = tpt_thermo_engine::export::rankine_sheet(&s.input, &s.result, &s.title);
            download("rankine-cycle.md", "text/markdown", &md);
            Ok(())
        }
    }
}

/// Exports the state points (or a saturation table for sat lookups) as CSV.
pub fn export_calc_csv(calc: &CurrentCalc) -> Result<(), String> {
    match calc {
        CurrentCalc::None => Err("Nothing to export yet — run a lookup or a cycle first.".into()),
        CurrentCalc::Lookup(solved) => {
            let fluid = solved.fluid;
            match solved.mode {
                crate::app::Mode::SatByTemperature | crate::app::Mode::SatByPressure => {
                    // A small saturation table bracketing the looked-up point.
                    let t_center = solved.t_sat_c.unwrap_or(20.0);
                    let (t_lo, t_hi) = fluids::saturation_range(fluid);
                    let csv = tpt_thermo_engine::export::saturation_table_csv(
                        fluid,
                        (t_center - 10.0).max(t_lo - 273.15),
                        (t_center + 10.0).min(t_hi - 273.15),
                        2.5,
                    );
                    download(&format!("{}-saturation.csv", fluid.id), "text/csv", &csv);
                }
                crate::app::Mode::Superheated => {
                    let st = lookup_point(solved)?;
                    let csv = tpt_thermo_engine::export::state_table_csv(&[
                        tpt_thermo_engine::cycle::CyclePoint {
                            label: format!("{} point", fluid.name),
                            state: st,
                        },
                    ]);
                    download(&format!("{}-point.csv", fluid.id), "text/csv", &csv);
                }
            }
            Ok(())
        }
        CurrentCalc::Ref(s) => {
            let csv = tpt_thermo_engine::export::state_table_csv(&s.result.points);
            download("refrigeration-cycle-points.csv", "text/csv", &csv);
            Ok(())
        }
        CurrentCalc::Rankine(s) => {
            let csv = tpt_thermo_engine::export::state_table_csv(&s.result.points);
            download("rankine-cycle-points.csv", "text/csv", &csv);
            Ok(())
        }
    }
}

/// Blob + object-URL + click download.
pub fn download(filename: &str, mime: &str, text: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(blob_parts) = js_sys::Array::of1(&JsValue::from_str(text)).dyn_into::<js_sys::Array>()
    else {
        return;
    };
    let mut props = web_sys::BlobPropertyBag::new();
    props.set_type(mime);
    let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&blob_parts, &props) else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };
    if let Some(document) = window.document() {
        if let Ok(anchor) = document.create_element("a") {
            let _ = anchor.set_attribute("href", &url);
            let _ = anchor.set_attribute("download", filename);
            let body = document.body();
            if let Some(body) = body {
                let _ = body.append_child(&anchor);
                if let Ok(a) = anchor.dyn_into::<web_sys::HtmlAnchorElement>() {
                    a.click();
                    let _ = body.remove_child(&a.into()).ok();
                }
            }
        }
    }
    let _ = web_sys::Url::revoke_object_url(&url);
}

/// Removes one saved calc (called from the app dispatch).
pub fn delete_calc(id: &str) {
    library::delete(id);
}

fn label<Msg2>(field: &mut ContainerBuilder<Msg2>, text: impl Into<String>) {
    field.text(text).class("thermo-label");
}

fn fmt(v: f64, digits: usize) -> String {
    if v.is_nan() {
        return "—".to_string();
    }
    format!("{v:.digits$}")
}
