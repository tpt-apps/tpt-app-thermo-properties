//! Free WASM edition UI (tpt-appfront-dom).
//!
//! Rendering architecture (mirrors the fea-lite/cutlist-optimizer split):
//! - the **input panel is mounted once** (`tpt_appfront_dom::mount`) — typing
//!   never loses focus; field values are read straight from the DOM when
//!   Lookup is pressed;
//! - the **results section re-renders fine-grained** via
//!   `tpt_appfront_dom::render` driven by the `outcome` Signal.

use std::rc::Rc;

use tpt_appfront_core::{Signal, UITree};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[derive(Debug, Clone)]
enum Msg {
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
.thermo-hint{color:var(--th-muted);font-size:.8rem;margin:0 0 10px}
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

.thermo-results-view{margin:4px 0 0}
.thermo-summary{margin:0 0 14px;padding:14px 16px;background:var(--th-accent-soft);border-radius:10px}
.thermo-headline{display:block;font-weight:700;font-size:1.05rem;margin:0 0 4px;color:var(--th-fg)}
.thermo-detail{display:block;color:var(--th-muted);font-size:.83rem;margin:0}
.thermo-engine{display:block;color:var(--th-muted);font-size:.8rem;margin:6px 0 0;font-style:italic}
.thermo-app table{width:100%;border-collapse:collapse;font-size:.85rem;margin-top:4px}
.thermo-app th{text-align:left;padding:8px;color:var(--th-muted);font-size:.7rem;font-weight:700;text-transform:uppercase;letter-spacing:.03em;border-bottom:2px solid var(--th-border)}
.thermo-app td{text-align:left;padding:8px;border-bottom:1px solid var(--th-border)}
.thermo-app tbody tr:hover{background:var(--th-panel)}

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

    let dispatch: Rc<dyn Fn(Msg)> = Rc::new(|msg| match msg {
        Msg::VisitSite(url) => visit_site(&url),
    });

    let shell = shell_tree();
    let root = tpt_appfront_dom::mount(container, &shell, dispatch.clone())?;
    std::mem::forget(root);

    // Reactive results section into its placeholder (proves the render path
    // this scaffold's real lookup results will flow through).
    let outcome: Signal<Outcome> = Signal::new(Outcome::Idle);
    let results_el = container
        .query_selector(".thermo-results")?
        .ok_or_else(|| JsValue::from_str("results placeholder missing"))?;
    let handle = tpt_appfront_dom::render(
        &results_el,
        Rc::new(move || results_view(outcome.clone())),
        dispatch,
    )?;
    std::mem::forget(handle);
    Ok(())
}

/// The results section's current content.
#[derive(Clone)]
enum Outcome {
    Idle,
}

/// The reactive results section (re-rendered on every `outcome` change).
fn results_view(outcome: Signal<Outcome>) -> UITree<Msg> {
    let _outcome = outcome.get();
    let mut root_builder = tpt_appfront_core::ContainerBuilder::<Msg>::new();
    root_builder
        .container(|c| {
            c.container(|summary| {
                styled_text(
                    summary,
                    "Scaffold mounted \u{2014} solver integration lands in Phase 1.",
                    "thermo-headline",
                );
                styled_text(
                    summary,
                    "This placeholder is the reactive results section.",
                    "thermo-detail",
                );
            })
            .class("thermo-summary");
        })
        .class("thermo-results-view");
    root_builder
        .into_only_child()
        .expect("results view builds exactly one root container")
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

/// The mounted-once shell: header, input card, results placeholder, upsell.
fn shell_tree() -> UITree<Msg> {
    UITree::container(|c| {
        c.container(|header| {
            header.text("TP").class("thermo-brand");
            header.container(|group| {
                group.container(|row| {
                    row.heading(2, "Thermo Properties");
                    row.text("Free edition").class("thermo-chip");
                })
                .class("thermo-title-row");
                group.text("Refrigerant property lookups for HVAC, refrigeration and process work — solved locally in your browser. Units: \u{b0}C, kPa, kJ/kg.")
                    .class("thermo-sub");
            })
            .class("thermo-title-group");
        })
        .class("thermo-header");

        c.container(|shell| {
            shell.container(|sidebar| {
                sidebar.container(|card| {
                    card.heading(3, "Property lookup");
                    card.text("Phase 0 scaffold — fluid picker and lookup fields land in Phase 2.")
                        .class("thermo-hint");
                })
                .class("thermo-card");
            })
            .class("thermo-sidebar");

            shell.container(|main| {
                // Filled reactively by `render` + `results_view` (see mount_app).
                main.container(|_results| {}).class("thermo-results");

                main.container(|upsell| {
                    upsell.heading(3, "Need cycle analysis, saved fluids, or exportable calc sheets?");
                    upsell.text("The Pro edition adds full refrigeration/Rankine cycle analysis, a saved fluid property library, and exportable calculation sheets for design documentation.")
                        .class("thermo-hint");
                    upsell.container(|links| {
                        links.text("Coming soon \u{2014} watch ")
                            .class("thermo-hint");
                        links.text("tptsolutions.co.nz")
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

fn styled_text<Msg2>(
    builder: &mut tpt_appfront_core::ContainerBuilder<Msg2>,
    text: impl Into<String>,
    class: &'static str,
) {
    builder.text(text).class(class);
}

fn upsert_style(document: &web_sys::Document, id: &str, css: &str) {
    if let Some(existing) = document.get_element_by_id(id) {
        if let Ok(style_el) = existing.dyn_into::<web_sys::HtmlElement>() {
            let _ = style_el.set_text_content(Some(css));
            return;
        }
    }
    if let Ok(el) = document.create_element("style") {
        let _ = el.set_attribute("id", id);
        let _ = el.set_text_content(Some(css));
        if let Some(head) = document.head() {
            let _ = head.append_child(&el);
        }
    }
}
