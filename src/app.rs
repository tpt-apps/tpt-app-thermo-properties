//! Free WASM edition UI (tpt-appfront-dom), single-point property lookup.
//!
//! Rendering architecture (mirrors the fea-lite/cutlist-optimizer split):
//! - the **input panel is mounted once** (`tpt_appfront_dom::mount`) — typing
//!   never loses focus; field values are read straight from the DOM when
//!   Lookup is pressed;
//! - the **results section re-renders fine-grained** via
//!   `tpt_appfront_dom::render` driven by the `outcome` Signal;
//! - lookup-mode switching shows/hides the temperature/pressure field rows by
//!   toggling `display` on their placeholder containers (same trick as the
//!   cutlist's optional rows — the static shell stays static).

use std::rc::Rc;

use tpt_appfront_core::{Signal, UITree};
use tpt_thermo_engine::fluids::{self, Fluid, FLUIDS};
use tpt_thermo_engine::saturation;
use tpt_thermo_engine::state::{self, State};
use wasm_bindgen::prelude::*;
#[cfg(all(target_arch = "wasm32", feature = "pro"))]
use crate::{library, pro, viewer};

// (pro) Which section the user solved last — Save/Export capture that one
// when several results are on screen. Wasm is single-threaded; a plain
// thread-local cell is the cheapest correct marker.
#[cfg(all(target_arch = "wasm32", feature = "pro"))]
thread_local! {
    static LAST_SOLVED: std::cell::Cell<&'static str> = const { std::cell::Cell::new("lookup") };
}

/// (pro) Records the most recently solved section ("lookup" | "ref" | "rankine").
#[cfg(all(target_arch = "wasm32", feature = "pro"))]
pub fn note_solved(section: &'static str) {
    LAST_SOLVED.with(|cell| cell.set(section));
}

/// (pro) The most recently solved section.
#[cfg(all(target_arch = "wasm32", feature = "pro"))]
pub fn last_solved() -> &'static str {
    LAST_SOLVED.with(|cell| cell.get())
}
use wasm_bindgen::JsCast;

/// The results section's current content.
#[derive(Clone)]
pub enum Outcome {
    Idle,
    /// Field validation / lookup errors, rendered as an issue list.
    Invalid(Vec<String>),
    Solved(Rc<Solved>),
}

/// One successful lookup: the saturation context (when applicable) plus the
/// evaluated point.
pub struct Solved {
    pub fluid: &'static Fluid,
    /// Mode the lookup ran in (drives the property table's shape).
    pub mode: Mode,
    /// Saturation temperature [°C] when the point sits on the line.
    pub t_sat_c: Option<f64>,
    /// Saturation pressure [kPa] when the point sits on the line.
    pub p_sat_kpa: Option<f64>,
    /// The evaluated point (superheat) or `None` for pure saturation tables.
    pub point: Option<State>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    SatByTemperature,
    SatByPressure,
    Superheated,
}

impl Mode {
    fn id(&self) -> &'static str {
        match self {
            Mode::SatByTemperature => "sat-t",
            Mode::SatByPressure => "sat-p",
            Mode::Superheated => "super",
        }
    }
    fn from_id(id: &str) -> Mode {
        match id {
            "sat-p" => Mode::SatByPressure,
            "super" => Mode::Superheated,
            _ => Mode::SatByTemperature,
        }
    }
    fn label(&self) -> &'static str {
        match self {
            Mode::SatByTemperature => "Saturation at a temperature",
            Mode::SatByPressure => "Saturation at a pressure",
            Mode::Superheated => "Superheated vapor point (T, p)",
        }
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    Lookup,
    /// Lookup-mode select changed — show/hide the matching input rows.
    ModeChanged(String),
    /// Fill the form with a worked example.
    Example,
    /// (pro) Solve the refrigeration-cycle card.
    #[cfg(feature = "pro")]
    SolveRefCycle,
    /// (pro) Solve the Rankine-cycle card.
    #[cfg(feature = "pro")]
    SolveRankine,
    /// (pro) Save the current calc into the library.
    #[cfg(feature = "pro")]
    SaveCurrent,
    /// (pro) Delete the saved calc with this id.
    #[cfg(feature = "pro")]
    DeleteCalc(String),
    /// (pro) Re-run the saved calc with this id.
    #[cfg(feature = "pro")]
    LoadCalc(String),
    /// (pro) Export the current calc as a Markdown calculation sheet.
    #[cfg(feature = "pro")]
    ExportSheet,
    /// (pro) Export the current calc's state points as CSV.
    #[cfg(feature = "pro")]
    ExportCsv,
    /// Open a TPT Solutions URL — in a new browser tab for the in-browser
    /// free edition, or via the desktop host's system-browser IPC action for
    /// the Pro desktop shell (whose webview would otherwise navigate itself
    /// away from the local `app://` bundle with no way back). See
    /// [`visit_site`].
    VisitSite(String),
}

const BASE_CSS: &str = r#"
.thermo-app{--th-bg:#f7f8fa;--th-surface:#ffffff;--th-panel:#f2f4f7;--th-fg:#181d27;--th-muted:#667085;--th-border:#e3e7ee;--th-accent:#b3541e;--th-accent-2:#8f3f12;--th-accent-soft:#fbf0e8;--th-accent-fg:#ffffff;--th-err:#b3261e;--th-err-bg:#fbeeec;--th-shadow:0 1px 2px rgba(16,24,40,.04),0 1px 3px rgba(16,24,40,.06);max-width:1360px;margin:0 auto;background:var(--th-bg);color:var(--th-fg);font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;font-size:14.5px;line-height:1.55;padding:0 4px 32px}
@media(prefers-color-scheme:dark){.thermo-app{--th-bg:#0b0f16;--th-surface:#141a24;--th-panel:#161d2a;--th-fg:#e8ebf0;--th-muted:#98a2b3;--th-border:#262f3d;--th-accent:#e08a4e;--th-accent-2:#f0a86a;--th-accent-soft:#2a1c10;--th-accent-fg:#1c0f04;--th-err:#f2b8b5;--th-err-bg:#2c1a1c;--th-shadow:0 1px 2px rgba(0,0,0,.3)}}
.thermo-app *{box-sizing:border-box}
.thermo-app h2{font-size:1.36rem;margin:0;font-weight:700;letter-spacing:-.01em}
.thermo-app h3{font-size:1rem;margin:0 0 6px;font-weight:700}

.thermo-header{display:flex;align-items:flex-start;gap:14px;padding:22px 2px 18px;border-bottom:1px solid var(--th-border);margin-bottom:22px}
.thermo-brand{flex:0 0 auto;width:42px;height:42px;border-radius:11px;background:linear-gradient(135deg,var(--th-accent),var(--th-accent-2));display:flex;align-items:center;justify-content:center;color:var(--th-accent-fg);font-weight:800;font-size:.82rem;letter-spacing:.01em;box-shadow:var(--th-shadow)}
.thermo-title-group{flex:1;min-width:0}
.thermo-title-row{display:flex;align-items:center;gap:10px;flex-wrap:wrap}
.thermo-chip{display:inline-flex;align-items:center;padding:3px 10px;border-radius:999px;font-size:.66rem;font-weight:800;letter-spacing:.05em;text-transform:uppercase;background:var(--th-accent-soft);color:var(--th-accent)}
.thermo-sub{color:var(--th-muted);margin:5px 0 0;font-size:.88rem}

.thermo-shell{display:grid;grid-template-columns:minmax(320px,380px) 1fr;gap:22px;align-items:start}
@media(max-width:900px){.thermo-shell{grid-template-columns:1fr}}
.thermo-sidebar{display:flex;flex-direction:column;gap:16px;position:sticky;top:16px;max-height:calc(100vh - 32px);overflow-y:auto;padding:2px 4px 12px 2px}
@media(max-width:900px){.thermo-sidebar{position:static;max-height:none;overflow:visible}}
.thermo-main{display:flex;flex-direction:column;gap:16px;min-width:0}

.thermo-card{background:var(--th-surface);border:1px solid var(--th-border);border-radius:14px;padding:16px 18px;box-shadow:var(--th-shadow)}
.thermo-hint{color:var(--th-muted);font-size:.8rem;margin:8px 0 0}
.thermo-row{display:flex;gap:12px;flex-wrap:wrap;margin-top:10px}
.thermo-field{display:flex;flex-direction:column;gap:5px}
.thermo-label{font-size:.72rem;color:var(--th-muted);font-weight:600;text-transform:uppercase;letter-spacing:.03em}
.thermo-input{width:110px;padding:7px 9px;border:1px solid var(--th-border);border-radius:8px;background:var(--th-bg);color:var(--th-fg);font:inherit;transition:border-color .12s,box-shadow .12s}
select.thermo-input{width:100%}
.thermo-input:focus{outline:none;border-color:var(--th-accent);box-shadow:0 0 0 3px var(--th-accent-soft)}

.thermo-actions{display:flex;gap:8px;flex-wrap:wrap;margin-top:14px;align-items:center}
.thermo-btn{padding:9px 16px;border-radius:9px;border:1px solid var(--th-border);background:var(--th-surface);color:var(--th-fg);font:inherit;font-weight:600;cursor:pointer;transition:border-color .12s,color .12s,transform .12s}
.thermo-btn:hover{border-color:var(--th-accent);color:var(--th-accent);transform:translateY(-1px)}
.thermo-primary{background:linear-gradient(135deg,var(--th-accent),var(--th-accent-2));border-color:transparent;color:var(--th-accent-fg);box-shadow:var(--th-shadow)}
.thermo-primary:hover{color:var(--th-accent-fg);filter:brightness(1.07)}

.thermo-results-view{display:flex;flex-direction:column;gap:16px}
.thermo-summary{margin:0;padding:14px 16px;background:var(--th-accent-soft);border-radius:10px}
.thermo-headline{display:block;font-weight:700;font-size:1.05rem;margin:0 0 4px;color:var(--th-fg)}
.thermo-detail{display:block;color:var(--th-muted);font-size:.83rem;margin:0}
.thermo-engine{display:block;color:var(--th-muted);font-size:.8rem;margin:6px 0 0;font-style:italic}
.thermo-issues:empty{display:none}
.thermo-issues{margin:0;padding:10px 12px;background:var(--th-err-bg);border:1px solid var(--th-err);border-radius:10px}
.thermo-issue{display:block;color:var(--th-err);font-size:.88rem;margin:2px 0}
.thermo-app table{width:100%;border-collapse:collapse;font-size:.85rem;margin-top:4px}
.thermo-app th{text-align:left;padding:8px;color:var(--th-muted);font-size:.7rem;font-weight:700;text-transform:uppercase;letter-spacing:.03em;border-bottom:2px solid var(--th-border)}
.thermo-app td{text-align:left;padding:8px;border-bottom:1px solid var(--th-border)}
.thermo-app tbody tr:hover{background:var(--th-panel)}
.thermo-fluidcard td:first-child{color:var(--th-muted)}

.thermo-ph-viewer canvas{width:100%;height:auto;display:block;border-radius:8px;background:var(--th-panel)}
.thermo-upsell{border-radius:14px;padding:16px 18px;background:linear-gradient(135deg,var(--th-accent-soft),transparent);border:1px dashed var(--th-accent);color:var(--th-fg);font-size:.88rem}
.thermo-upsell h3{margin:0 0 6px}
.thermo-upsell a{color:var(--th-accent);font-weight:700;text-decoration:none}
.thermo-upsell a:hover{text-decoration:underline}

.thermo-footer{display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:10px;margin-top:26px;padding-top:16px;border-top:1px solid var(--th-border);color:var(--th-muted);font-size:.82rem}
.thermo-footer-brand{font-weight:700;color:var(--th-fg)}
.thermo-footer-links{display:flex;flex-wrap:wrap;gap:4px 16px}
.thermo-footer-link{padding:0;border:none;background:none;color:var(--th-accent);font:inherit;font-weight:600;cursor:pointer}
.thermo-footer-link:hover{text-decoration:underline}
"#;

/// Default field values = the worked example (R-134a saturation at 0 °C).
mod defaults {
    pub const FLUID: &str = "r134a";
    pub const TEMP_C: &str = "0";
    pub const PRESS_KPA: &str = "292.8";
}

/// Mounts the whole free-edition app into `container` (the hub's container
/// div, or the standalone `#tpt-appfront-root` marker).
pub fn mount_app(container: &web_sys::Element) -> Result<(), JsValue> {
    let document = web_sys::window()
        .expect("no window")
        .document()
        .expect("no document");

    upsert_style(&document, "thermo-properties-styles", BASE_CSS);
    // `.thermo-app` on the mount root itself: every color/font/shadow custom
    // property lives here, so the whole theme depends on this class being
    // present on an ancestor of the tree the CSS above targets.
    let existing_class = container.get_attribute("class").unwrap_or_default();
    if !existing_class.split_whitespace().any(|c| c == "thermo-app") {
        let combined = if existing_class.is_empty() {
            "thermo-app".to_string()
        } else {
            format!("{existing_class} thermo-app")
        };
        let _ = container.set_attribute("class", &combined);
    }

    let outcome: Signal<Outcome> = Signal::new(Outcome::Idle);
    #[cfg(feature = "pro")]
    let ref_outcome: Signal<pro::CycleOutcome> = Signal::new(pro::CycleOutcome::Idle);
    #[cfg(feature = "pro")]
    let rankine_outcome: Signal<pro::RankineOutcome> = Signal::new(pro::RankineOutcome::Idle);
    #[cfg(feature = "pro")]
    let lib_outcome: Signal<pro::LibraryOutcome> =
        Signal::new(pro::LibraryOutcome::List(Rc::new((0, Vec::new()))));
    #[cfg(feature = "pro")]
    let lib_counter = std::rc::Rc::new(std::cell::Cell::new(0u64));

    let dispatch: Rc<dyn Fn(Msg)> = {
        let container = container.clone();
        let outcome = outcome.clone();
        #[cfg(feature = "pro")]
        let ref_outcome = ref_outcome.clone();
        #[cfg(feature = "pro")]
        let rankine_outcome = rankine_outcome.clone();
        #[cfg(feature = "pro")]
        let lib_outcome = lib_outcome.clone();
        #[cfg(feature = "pro")]
        let lib_counter = lib_counter.clone();
        Rc::new(move |msg| match msg {
            Msg::Lookup => {
                run_lookup(&container, &outcome);
                #[cfg(feature = "pro")]
                note_solved("lookup");
            }
            Msg::ModeChanged(id) => apply_mode(&container, Mode::from_id(&id)),
            Msg::Example => {
                apply_example(&container);
                run_lookup(&container, &outcome);
            }
            Msg::VisitSite(url) => visit_site(&url),
            #[cfg(feature = "pro")]
            Msg::SolveRefCycle => {
                pro::solve_ref_cycle(&container, &ref_outcome);
                note_solved("ref");
            }
            #[cfg(feature = "pro")]
            Msg::SolveRankine => {
                pro::solve_rankine(&container, &rankine_outcome);
                note_solved("rankine");
            }
            #[cfg(feature = "pro")]
            Msg::SaveCurrent => {
                let lookup = outcome.get();
                let refr = ref_outcome.get();
                let rank = rankine_outcome.get();
                let calc = pro::capture_current(&container, &lookup, &refr, &rank);
                match pro::save_current(&calc) {
                    Ok(_) => pro::refresh_library(&lib_outcome, lib_counter.get() + 1),
                    Err(msg) => alert(&msg),
                }
            }
            #[cfg(feature = "pro")]
            Msg::DeleteCalc(id) => {
                pro::delete_calc(&id);
                pro::refresh_library(&lib_outcome, lib_counter.get() + 1);
            }
            #[cfg(feature = "pro")]
            Msg::LoadCalc(id) => {
                if let Some(entry) = library::list().into_iter().find(|e| e.id == id) {
                    pro::load_calc(&entry, &container, &outcome, &ref_outcome, &rankine_outcome);
                }
            }
            #[cfg(feature = "pro")]
            Msg::ExportSheet => {
                let lookup = outcome.get();
                let refr = ref_outcome.get();
                let rank = rankine_outcome.get();
                let calc = pro::capture_current(&container, &lookup, &refr, &rank);
                if let Err(msg) = pro::export_calc_sheet(&calc) {
                    alert(&msg);
                }
            }
            #[cfg(feature = "pro")]
            Msg::ExportCsv => {
                let lookup = outcome.get();
                let refr = ref_outcome.get();
                let rank = rankine_outcome.get();
                let calc = pro::capture_current(&container, &lookup, &refr, &rank);
                if let Err(msg) = pro::export_calc_csv(&calc) {
                    alert(&msg);
                }
            }
        })
    };

    // 1. Static shell — mounted exactly once.
    let shell = shell_tree();
    let root = tpt_appfront_dom::mount(container, &shell, dispatch.clone())?;
    std::mem::forget(root);

    // 1b. Apply the initial lookup-mode visibility (sat-by-T hides the
    // pressure row) before first paint of the results.
    let initial_mode = read_field(container, ".thermo-mode");
    apply_mode(container, Mode::from_id(&initial_mode));

    // 2. Reactive results section into its placeholder.
    let results_el = container
        .query_selector(".thermo-results")?
        .ok_or_else(|| JsValue::from_str("results placeholder missing"))?;
    let outcome_for_render = outcome.clone();
    let handle = tpt_appfront_dom::render(
        &results_el,
        Rc::new(move || results_view(outcome_for_render.clone())),
        dispatch.clone(),
    )?;
    std::mem::forget(handle);

    // 3. (pro) Cycle sections, P-h viewer and library.
    #[cfg(feature = "pro")]
    mount_pro_sections(
        container,
        &dispatch,
        &ref_outcome,
        &rankine_outcome,
        &lib_outcome,
    )?;

    Ok(())
}

/// (pro) Wires the cycle cards' reactive sections, the P-h canvas and the
/// saved-calc list. Called once from [`mount_app`].
#[cfg(feature = "pro")]
fn mount_pro_sections(
    container: &web_sys::Element,
    dispatch: &Rc<dyn Fn(Msg)>,
    ref_outcome: &Signal<pro::CycleOutcome>,
    rankine_outcome: &Signal<pro::RankineOutcome>,
    lib_outcome: &Signal<pro::LibraryOutcome>,
) -> Result<(), JsValue> {
    let document = web_sys::window()
        .expect("no window")
        .document()
        .expect("no document");

    // Refrigeration results section.
    let el = container
        .query_selector(".thermo-ref-results")?
        .ok_or_else(|| JsValue::from_str("ref results placeholder missing"))?;
    let outcome_for_render = ref_outcome.clone();
    let handle = tpt_appfront_dom::render(
        &el,
        Rc::new(move || pro::ref_results_view(outcome_for_render.clone())),
        dispatch.clone(),
    )?;
    std::mem::forget(handle);

    // Rankine results section.
    let el = container
        .query_selector(".thermo-rankine-results")?
        .ok_or_else(|| JsValue::from_str("rankine results placeholder missing"))?;
    let outcome_for_render = rankine_outcome.clone();
    let handle = tpt_appfront_dom::render(
        &el,
        Rc::new(move || pro::rankine_results_view(outcome_for_render.clone())),
        dispatch.clone(),
    )?;
    std::mem::forget(handle);

    // Saved-calc list.
    let el = container
        .query_selector(".thermo-lib-list")?
        .ok_or_else(|| JsValue::from_str("library placeholder missing"))?;
    let outcome_for_render = lib_outcome.clone();
    let handle = tpt_appfront_dom::render(
        &el,
        Rc::new(move || pro::library_view(outcome_for_render.clone())),
        dispatch.clone(),
    )?;
    std::mem::forget(handle);
    pro::refresh_library(lib_outcome, 0);

    // P-h canvas, redrawn from the refrigeration outcome.
    let holder = container
        .query_selector(".thermo-ph-viewer")?
        .ok_or_else(|| JsValue::from_str("P-h viewer placeholder missing"))?;
    let canvas: web_sys::HtmlCanvasElement = document
        .create_element("canvas")?
        .dyn_into()
        .map_err(|_| JsValue::from_str("canvas cast failed"))?;
    canvas.set_width(viewer::W as u32);
    canvas.set_height(viewer::H as u32);
    holder.append_child(&canvas)?;

    let container_for_css = container.clone();
    let ref_effect = ref_outcome.clone();
    let handle = tpt_appfront_core::create_effect(move || {
        if let pro::CycleOutcome::Solved(solved) = ref_effect.get() {
            let fg = read_css(&container_for_css, "--th-fg", "#1f2430");
            let muted = read_css(&container_for_css, "--th-muted", "#667085");
            let accent = read_css(&container_for_css, "--th-accent", "#b3541e");
            viewer::draw(&canvas, solved.fluid, &solved.result, &fg, &muted, &accent);
        }
    });
    std::mem::forget(handle);
    Ok(())
}

/// web-sys window (None off-DOM).
#[cfg(feature = "pro")]
fn window() -> Option<web_sys::Window> {
    web_sys::window()
}

/// Browser-computed CSS variable, with a light-theme fallback for the canvas.
#[cfg(feature = "pro")]
fn read_css(container: &web_sys::Element, name: &str, fallback: &str) -> String {
    window()
        .and_then(|w| w.get_computed_style(container).ok()?)
        .and_then(|style| style.get_property_value(name).ok())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

#[cfg(feature = "pro")]
fn alert(message: &str) {
    if let Some(window) = window() {
        window.alert_with_message(message);
    }
}

/// Pub wrappers for the helpers the pro module reuses.
#[cfg(feature = "pro")]
pub fn styled_text_pub<Msg2>(
    builder: &mut tpt_appfront_core::ContainerBuilder<Msg2>,
    text: impl Into<String>,
    class: &'static str,
) {
    styled_text(builder, text, class);
}

#[cfg(feature = "pro")]
pub fn read_field_pub(container: &web_sys::Element, selector: &str) -> String {
    read_field(container, selector)
}

#[cfg(feature = "pro")]
pub fn write_field_pub(container: &web_sys::Element, selector: &str, value: &str) {
    write_field(container, selector, value);
}

#[cfg(feature = "pro")]
pub fn apply_mode_pub(container: &web_sys::Element, mode_id: &str) {
    apply_mode(container, Mode::from_id(mode_id));
}

#[cfg(feature = "pro")]
pub fn run_lookup_pub(container: &web_sys::Element, outcome: &Signal<Outcome>) {
    run_lookup(container, outcome);
}

#[cfg(feature = "pro")]
pub fn delete_calc(id: &str) {
    library::delete(id);
}

fn visit_site(url: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let in_desktop_shell = window
        .location()
        .protocol()
        .map(|p| p == "app:")
        .unwrap_or(false);

    if in_desktop_shell {
        // Routed through the desktop host's allowlisted IPC bridge
        // (`window.__appfront.post`, injected by tpt-appfront-webview) so the
        // *native* process opens the OS default browser, leaving the app
        // window untouched.
        if let Ok(appfront) = js_sys::Reflect::get(&window, &JsValue::from_str("__appfront")) {
            if let Ok(post) = js_sys::Reflect::get(&appfront, &JsValue::from_str("post")) {
                if let Ok(post_fn) = post.dyn_into::<js_sys::Function>() {
                    let params = js_sys::Object::new();
                    let _ = js_sys::Reflect::set(
                        &params,
                        &JsValue::from_str("url"),
                        &JsValue::from_str(url),
                    );
                    let _ = post_fn.call2(&appfront, &JsValue::from_str("open_external"), &params);
                }
            }
        }
    } else {
        let _ = window.open_with_url_and_target(url, "_blank");
    }
}

/// The mounted-once shell: header, lookup card, results placeholder, upsell.
fn shell_tree() -> UITree<Msg> {
    UITree::container(|c| {
        c.container(|header| {
            header.text("TP").class("thermo-brand");
            header.container(|group| {
                group.container(|row| {
                    row.heading(2, "Thermo Properties");
                    #[cfg(not(feature = "pro"))]
                    row.text("Free edition").class("thermo-chip");
                    #[cfg(feature = "pro")]
                    row.text("Pro").class("thermo-chip");
                })
                .class("thermo-title-row");
                group.text("Single-point refrigerant property lookups for HVAC, refrigeration and process work — solved locally in your browser. Units: \u{b0}C, kPa, kJ/kg (h = 200 kJ/kg, s = 1 kJ/kg\u{b7}K at 0 \u{b0}C saturated liquid).")
                    .class("thermo-sub");
            })
            .class("thermo-title-group");
        })
        .class("thermo-header");

        c.container(|shell| {
            shell.container(|sidebar| {
                sidebar.container(|card| {
                    card.heading(3, "Property lookup");
                    card.container(|row| {
                        row.container(|field| {
                            label(field, "Working fluid");
                            field
                                .select(
                                    FLUIDS.iter().map(|f| (f.id, f.name)).collect::<Vec<_>>(),
                                    defaults::FLUID,
                                )
                                .class("thermo-input thermo-fluid")
                                .on_input(|_| Msg::Lookup);
                        })
                        .class("thermo-field");
                        row.container(|field| {
                            label(field, "Lookup mode");
                            field
                                .select(
                                    vec![
                                        (Mode::SatByTemperature.id(), Mode::SatByTemperature.label()),
                                        (Mode::SatByPressure.id(), Mode::SatByPressure.label()),
                                        (Mode::Superheated.id(), Mode::Superheated.label()),
                                    ],
                                    Mode::SatByTemperature.id(),
                                )
                                .class("thermo-input thermo-mode")
                                .on_input(Msg::ModeChanged);
                        })
                        .class("thermo-field");
                    })
                    .class("thermo-row");

                    // Temperature row (sat-by-T uses it as the saturation
                    // temperature; superheated as the vapor temperature).
                    card.container(|row| {
                        row.container(|field| {
                            label(field, "Temperature (\u{b0}C)");
                            field
                                .input(defaults::TEMP_C)
                                .class("thermo-input thermo-temp");
                        })
                        .class("thermo-field");
                    })
                    .class("thermo-row thermo-row-t");

                    // Pressure row.
                    card.container(|row| {
                        row.container(|field| {
                            label(field, "Pressure (kPa)");
                            field
                                .input(defaults::PRESS_KPA)
                                .class("thermo-input thermo-press");
                        })
                        .class("thermo-field");
                    })
                    .class("thermo-row thermo-row-p");

                    card.text("Saturation modes take one of temperature or pressure; the superheated mode takes both.")
                        .class("thermo-hint");

                    card.container(|actions| {
                        actions.button("Look up properties")
                            .class("thermo-btn thermo-primary")
                            .on_click(Msg::Lookup);
                        actions.button("Load example")
                            .class("thermo-btn")
                            .on_click(Msg::Example);
                    })
                    .class("thermo-actions");
                })
                .class("thermo-card");

                #[cfg(feature = "pro")]
                pro::sidebar_cards(sidebar);
            })
            .class("thermo-sidebar");

            shell.container(|main| {
                // Filled reactively by `render` + `results_view`.
                main.container(|_results| {}).class("thermo-results");

                #[cfg(feature = "pro")]
                pro::main_sections(main);

                #[cfg(not(feature = "pro"))]
                main.container(|upsell| {
                    upsell.heading(3, "Need cycle analysis, saved fluids, or exportable calc sheets?");
                    upsell.text("The Pro desktop edition adds full refrigeration and Rankine cycle analysis (COP, work, heat duty, discharge temperatures), a saved fluid property library, and exportable calculation sheets for design documentation \u{2014} a one-time purchase, no subscription.")
                        .class("thermo-hint");
                    upsell.container(|links| {
                        links
                            .text("See tptsolutions.co.nz")
                            .class("thermo-footer-link")
                            .on_click(Msg::VisitSite(
                                "https://tptsolutions.co.nz/tools/thermo-properties".to_string(),
                            ));
                        links
                            .text("\u{2192} full details and pricing")
                            .class("thermo-footer-link")
                            .on_click(Msg::VisitSite(
                                "https://tptsolutions.co.nz/tools/thermo-properties".to_string(),
                            ));
                    })
                    .class("thermo-footer-links");
                })
                .class("thermo-upsell");
            })
            .class("thermo-main");
        })
        .class("thermo-shell");

        c.container(|footer| {
            footer.text("TPT Solutions").class("thermo-footer-brand");
            footer.container(|links| {
                links
                    .text("tptsolutions.co.nz")
                    .class("thermo-footer-link")
                    .on_click(Msg::VisitSite("https://tptsolutions.co.nz".to_string()));
            })
            .class("thermo-footer-links");
        })
        .class("thermo-footer");
    })
}

/// Shows the input rows matching the selected lookup mode.
fn apply_mode(container: &web_sys::Element, mode: Mode) {
    let show = |selector: &str, visible: bool| {
        if let Some(el) = container.query_selector(selector).ok().flatten() {
            let _ = el.set_attribute("style", if visible { "display:flex" } else { "display:none" });
        }
    };
    show(".thermo-row-t", matches!(mode, Mode::SatByTemperature | Mode::Superheated));
    show(".thermo-row-p", matches!(mode, Mode::SatByPressure | Mode::Superheated));
}

/// Fills the form with the worked example: R-134a saturation at 0 °C.
fn apply_example(container: &web_sys::Element) {
    write_field(container, ".thermo-fluid", defaults::FLUID);
    write_field(container, ".thermo-mode", Mode::SatByTemperature.id());
    apply_mode(container, Mode::SatByTemperature);
    write_field(container, ".thermo-temp", defaults::TEMP_C);
    write_field(container, ".thermo-press", defaults::PRESS_KPA);
}

/// Reads the form, validates, runs the lookup, and publishes the outcome.
fn run_lookup(container: &web_sys::Element, outcome: &Signal<Outcome>) {
    let mut issues = Vec::new();

    let fluid_id = read_field(container, ".thermo-fluid");
    let mode = Mode::from_id(&read_field(container, ".thermo-mode"));
    let fluid = match fluids::find(&fluid_id) {
        Some(f) => f,
        None => {
            issues.push("Pick a working fluid from the list.".to_string());
            outcome.set(Outcome::Invalid(issues));
            return;
        }
    };

    let t_input = read_field(container, ".thermo-temp");
    let p_input = read_field(container, ".thermo-press");
    let t_val = parse_num(&t_input, "Temperature", &mut issues);
    let p_val = parse_num(&p_input, "Pressure", &mut issues);

    if !issues.is_empty() {
        outcome.set(Outcome::Invalid(issues));
        return;
    }

    let solved = match mode {
        Mode::SatByTemperature => {
            let t_c = t_val.unwrap();
            let psat = saturation::psat(fluid, t_c);
            if psat.is_nan() {
                let (t_lo, t_hi) = fluids::saturation_range(fluid);
                issues.push(format!(
                    "{} °C is outside {}'s correlation range ({:.0} to {:.0} °C).",
                    t_c,
                    fluid.name,
                    t_lo - 273.15,
                    t_hi - 273.15
                ));
                outcome.set(Outcome::Invalid(issues));
                return;
            }
            Solved {
                fluid,
                mode,
                t_sat_c: Some(t_c),
                p_sat_kpa: Some(psat),
                point: None,
            }
        }
        Mode::SatByPressure => {
            let p_kpa = p_val.unwrap();
            if p_kpa <= 0.0 {
                issues.push("Pressure must be positive.".to_string());
                outcome.set(Outcome::Invalid(issues));
                return;
            }
            let (t_lo, t_hi) = fluids::saturation_range(fluid);
            let psat_lo = saturation::psat(fluid, t_lo - 273.15);
            let psat_hi = saturation::psat(fluid, t_hi - 273.15);
            if !(psat_lo..=psat_hi).contains(&p_kpa) {
                issues.push(format!(
                    "{} kPa is outside {}'s saturation range ({:.1} to {:.1} kPa).",
                    p_kpa, fluid.name, psat_lo, psat_hi
                ));
                outcome.set(Outcome::Invalid(issues));
                return;
            }
            let t_sat = saturation::tsat(fluid, p_kpa);
            Solved {
                fluid,
                mode,
                t_sat_c: Some(t_sat),
                p_sat_kpa: Some(p_kpa),
                point: None,
            }
        }
        Mode::Superheated => {
            let t_c = t_val.unwrap();
            let p_kpa = p_val.unwrap();
            match state::state_at(fluid, t_c, p_kpa) {
                Ok(st) => Solved {
                    fluid,
                    mode,
                    t_sat_c: None,
                    p_sat_kpa: None,
                    point: Some(st),
                },
                Err(e) => {
                    issues.push(e.0);
                    outcome.set(Outcome::Invalid(issues));
                    return;
                }
            }
        }
    };

    outcome.set(Outcome::Solved(Rc::new(solved)));
}

fn parse_num(raw: &str, label: &str, issues: &mut Vec<String>) -> Option<f64> {
    match raw.trim().parse::<f64>() {
        Ok(v) if v.is_finite() => Some(v),
        _ => {
            issues.push(format!("{label} must be a number (got \u{201c}{raw}\u{201d})."));
            None
        }
    }
}

fn read_field(container: &web_sys::Element, selector: &str) -> String {
    let Some(el) = container.query_selector(selector).ok().flatten() else {
        return String::new();
    };
    // Inputs and selects both carry `.value` but are unrelated web-sys types.
    if let Ok(input) = el.clone().dyn_into::<web_sys::HtmlInputElement>() {
        return input.value();
    }
    if let Ok(select) = el.dyn_into::<web_sys::HtmlSelectElement>() {
        return select.value();
    }
    String::new()
}

fn write_field(container: &web_sys::Element, selector: &str, value: &str) {
    let Some(el) = container.query_selector(selector).ok().flatten() else {
        return;
    };
    if let Ok(input) = el.clone().dyn_into::<web_sys::HtmlInputElement>() {
        input.set_value(value);
    } else if let Ok(select) = el.dyn_into::<web_sys::HtmlSelectElement>() {
        select.set_value(value);
    }
}

/// The reactive results section (re-rendered on every `outcome` change).
fn results_view(outcome: Signal<Outcome>) -> UITree<Msg> {
    let outcome = outcome.get();
    let mut root_builder = tpt_appfront_core::ContainerBuilder::<Msg>::new();
    root_builder
        .container(|c| {
            c.container(|summary| {
                let (headline, detail) = match &outcome {
                    Outcome::Idle => (
                        "Pick a fluid and press Look up properties.".to_string(),
                        "Or load the worked example for a one-click first run.".to_string(),
                    ),
                    Outcome::Invalid(issues) => (
                        format!(
                            "Check {} input issue{}:",
                            issues.len(),
                            if issues.len() == 1 { "" } else { "s" }
                        ),
                        String::new(),
                    ),
                    Outcome::Solved(solved) => match solved.mode {
                        Mode::SatByTemperature => {
                            let t = solved.t_sat_c.unwrap();
                            let p = solved.p_sat_kpa.unwrap();
                            (
                                format!(
                                    "{} at {} °C saturates at {} kPa",
                                    solved.fluid.name,
                                    fmt_g(t),
                                    fmt_g(p)
                                ),
                                "Full saturation properties below.".to_string(),
                            )
                        }
                        Mode::SatByPressure => {
                            let t = solved.t_sat_c.unwrap();
                            let p = solved.p_sat_kpa.unwrap();
                            (
                                format!(
                                    "{} at {} kPa boils at {} °C",
                                    solved.fluid.name,
                                    fmt_g(p),
                                    fmt_g(t)
                                ),
                                "Full saturation properties below.".to_string(),
                            )
                        }
                        Mode::Superheated => {
                            let st = solved.point.as_ref().unwrap();
                            (
                                format!(
                                    "{} · {} region at {} °C, {} kPa",
                                    solved.fluid.name,
                                    st.region.label(),
                                    fmt_g(st.t_c),
                                    fmt_g(st.p_kpa)
                                ),
                                format!("h = {} kJ/kg · s = {} kJ/(kg·K)", fmt_g(st.h), fmt_g(st.s)),
                            )
                        }
                    },
                };
                styled_text(summary, headline, "thermo-headline");
                if !detail.is_empty() {
                    styled_text(summary, detail, "thermo-detail");
                }
                styled_text(
                    summary,
                    "Engine: corresponding-states correlations calibrated to published reference anchors (see the accuracy note below).",
                    "thermo-engine",
                );
            })
            .class("thermo-summary");

            c.container(|issues| {
                if let Outcome::Invalid(list) = &outcome {
                    for issue in list {
                        styled_text(issues, issue.clone(), "thermo-issue");
                    }
                }
            })
            .class("thermo-issues");

            if let Outcome::Solved(solved) = &outcome {
                // Saturation property table.
                if let (Some(t_sat), Some(p_sat)) = (solved.t_sat_c, solved.p_sat_kpa) {
                    let f = solved.fluid;
                    let rows = vec![
                        sat_row("Saturation pressure", &fmt_g(p_sat), "kPa"),
                        sat_row("Saturation temperature", &fmt_g(t_sat), "°C"),
                        sat_row("Liquid density ρf", &fmt_g(saturation::rho_liquid(f, t_sat)), "kg/m³"),
                        sat_row("Vapor density ρg", &fmt_g(saturation::rho_vapor(f, t_sat)), "kg/m³"),
                        sat_row("Liquid enthalpy hf", &fmt_g(saturation::hf(f, t_sat)), "kJ/kg"),
                        sat_row("Vapor enthalpy hg", &fmt_g(saturation::hg(f, t_sat)), "kJ/kg"),
                        sat_row("Latent heat hfg", &fmt_g(saturation::hfg(f, t_sat)), "kJ/kg"),
                        sat_row("Liquid entropy sf", &fmt_g(saturation::sf(f, t_sat)), "kJ/(kg·K)"),
                        sat_row("Vapor entropy sg", &fmt_g(saturation::sg(f, t_sat)), "kJ/(kg·K)"),
                    ];
                    c.data_grid(["Property", "Value", "Unit"], rows);
                }

                // Superheated point table.
                if let Some(st) = &solved.point {
                    let rows = vec![
                        sat_row("Temperature", &fmt_g(st.t_c), "°C"),
                        sat_row("Pressure", &fmt_g(st.p_kpa), "kPa"),
                        sat_row("Specific enthalpy h", &fmt_g(st.h), "kJ/kg"),
                        sat_row("Specific entropy s", &fmt_g(st.s), "kJ/(kg·K)"),
                        sat_row("Density ρ", &fmt_g(st.rho), "kg/m³"),
                        sat_row(
                            "Specific volume v",
                            &fmt_g(1.0 / st.rho),
                            "m³/kg",
                        ),
                        sat_row("Region", st.region.label(), ""),
                    ];
                    c.data_grid(["Property", "Value", "Unit"], rows);
                }

                // Fluid constants card.
                let f = solved.fluid;
                c.container(|card| {
                    card.heading(3, "Fluid constants");
                    let rows = vec![
                        sat_row("Chemistry", f.chemistry, ""),
                        sat_row("Molar mass", &fmt_g(f.molar_mass), "kg/kmol"),
                        sat_row("Critical temperature", &fmt_g(f.t_crit - 273.15), "°C"),
                        sat_row("Critical pressure", &fmt_g(f.p_crit), "kPa"),
                        sat_row("Normal boiling point", &fmt_g(f.t_boil - 273.15), "°C"),
                    ];
                    card.data_grid(["Constant", "Value", "Unit"], rows);
                })
                .class("thermo-fluidcard");

                c.text(
                    "Accuracy: saturation pressure within ~1.5% of the calibration anchors, saturation temperature ~0.3 K, liquid density ~2%, latent heat ~2-3% (ammonia ~3%), vapor density ~3-6%. For preliminary design and cross-checking — verify final selections against REFPROP/NIST tables and manufacturer data.",
                )
                .class("thermo-hint");
            }
        })
        .class("thermo-results-view");
    root_builder
        .into_only_child()
        .expect("results view builds exactly one root container")
}

fn sat_row(name: &str, value: &str, unit: &str) -> Vec<String> {
    vec![name.to_string(), value.to_string(), unit.to_string()]
}

/// Small uppercase label above an input, mirroring the reference apps.
fn label<Msg2>(field: &mut tpt_appfront_core::ContainerBuilder<Msg2>, text: impl Into<String>) {
    field.text(text).class("thermo-label");
}

fn styled_text<Msg2>(
    builder: &mut tpt_appfront_core::ContainerBuilder<Msg2>,
    text: impl Into<String>,
    class: &'static str,
) {
    // Class a wrapping container div (a bare text span would ignore the
    // block-level margins/layout the class assumes).
    let text = text.into();
    builder.container(move |c| { c.text(text); }).class(class);
}

/// Adaptive numeric formatting for results text.
fn fmt_g(v: f64) -> String {
    let a = v.abs();
    if a == 0.0 {
        return "0.00".to_string();
    }
    if a >= 1000.0 {
        format!("{v:.0}")
    } else if a >= 100.0 {
        format!("{v:.1}")
    } else if a >= 1.0 {
        format!("{v:.2}")
    } else if a >= 0.001 {
        format!("{v:.4}")
    } else {
        format!("{v:.2e}")
    }
}

fn upsert_style(document: &web_sys::Document, id: &str, css: &str) {
    if let Ok(Some(existing)) = document.query_selector(&format!("style#{id}")) {
        existing.set_text_content(Some(css));
        return;
    }
    let Some(head) = document.head() else {
        return;
    };
    if let Ok(style_el) = document.create_element("style") {
        let _ = style_el.set_attribute("id", id);
        style_el.set_text_content(Some(css));
        let _ = head.append_child(&style_el);
    }
}
