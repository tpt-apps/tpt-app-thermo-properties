//! (pro) P-h cycle diagram: the refrigeration cycle drawn on a log-pressure
//! vs enthalpy axes with the saturation dome, drawn straight through web-sys
//! (the DOM backend has no canvas node kind — same approach as fea-lite's
//! mesh viewer).

use tpt_thermo_engine::cycle::RefrigerationResult;
use tpt_thermo_engine::fluids::Fluid;
use tpt_thermo_engine::saturation;
use wasm_bindgen::JsCast;

/// Logical canvas size (CSS scales it to the card width).
pub const W: f64 = 960.0;
pub const H: f64 = 540.0;
/// Plot area inside the axes labels.
const PLOT: (f64, f64, f64, f64) = (64.0, 24.0, 916.0, 486.0); // x0, y0, x1, y1

/// Draws the diagram for one solved refrigeration cycle.
pub fn draw(canvas: &web_sys::HtmlCanvasElement, fluid: &Fluid, r: &RefrigerationResult, fg: &str, muted: &str, accent: &str) {
    let Some(ctx) = canvas
        .get_context("2d")
        .ok()
        .flatten()
        .and_then(|c| c.dyn_into::<web_sys::CanvasRenderingContext2d>().ok())
    else {
        return;
    };

    // Theme surface (canvas is transparent in the DOM; fill with panel color
    // read from CSS so dark/light both work).
    ctx.clear_rect(0.0, 0.0, W, H);

    // --- Axis ranges from the cycle + dome ---------------------------------
    let t_lo = r.points[0].state.t_c.min(r.points[3].state.t_c) - 8.0;
    let t_hi = r.points[2].state.t_c.max(r.points[1].state.t_c) + 12.0;
    let t_lo = t_lo.max(fluid.t_boil + 2.0 - 273.15);
    let t_hi = t_hi.min(fluid.t_crit - 273.15 - 2.0);

    let mut h_min = f64::INFINITY;
    let mut h_max = f64::NEG_INFINITY;
    for t in [t_lo, t_hi] {
        h_min = h_min.min(saturation::hf(fluid, t));
        h_max = h_max.max(saturation::hg(fluid, t));
    }
    for p in &r.points {
        h_min = h_min.min(p.state.h);
        h_max = h_max.max(p.state.h);
    }
    let pad = (h_max - h_min) * 0.06 + 5.0;
    h_min -= pad;
    h_max += pad;

    let p_lo = saturation::psat(fluid, t_lo);
    let p_hi = saturation::psat(fluid, t_hi).max(1.0);
    let lg_lo = p_lo.log10();
    let lg_hi = (p_hi * 1.15).log10();

    let (x0, y0, x1, y1) = PLOT;
    let sx = |h: f64| x0 + (h - h_min) / (h_max - h_min) * (x1 - x0);
    let sy = |p_kpa: f64| {
        let lg = p_kpa.max(1e-2).log10();
        y1 - (lg - lg_lo) / (lg_hi - lg_lo) * (y1 - y0)
    };

    // Light fill between the branches: one closed path (liquid up + vapor down).
    ctx.begin_path();
    for k in 0..=48 {
        let t = t_lo + (t_hi - t_lo) * (k as f64) / 48.0;
        let x = sx(saturation::hf(fluid, t));
        let y = sy(saturation::psat(fluid, t));
        if k == 0 {
            let _ = ctx.move_to(x, y);
        } else {
            let _ = ctx.line_to(x, y);
        }
    }
    for k in (0..=48).rev() {
        let t = t_lo + (t_hi - t_lo) * (k as f64) / 48.0;
        let _ = ctx.line_to(sx(saturation::hg(fluid, t)), sy(saturation::psat(fluid, t)));
    }
    ctx.close_path();
    ctx.set_fill_style_str("rgba(128,128,128,.12)");
    let _ = ctx.fill();
    // Dome outlines (liquid branch up, vapor branch down).
    ctx.set_stroke_style_str(&muted);
    ctx.set_line_width(1.2);
    for branch in [0, 1] {
        ctx.begin_path();
        for k in 0..=48 {
            let t = t_lo + (t_hi - t_lo) * (k as f64) / 48.0;
            let h = if branch == 0 {
                saturation::hf(fluid, t)
            } else {
                saturation::hg(fluid, t)
            };
            let x = sx(h);
            let y = sy(saturation::psat(fluid, t));
            if k == 0 {
                let _ = ctx.move_to(x, y);
            } else {
                let _ = ctx.line_to(x, y);
            }
        }
        let _ = ctx.stroke();
    }

    // --- Gridlines + axis labels -------------------------------------------
    ctx.set_font("11px system-ui, sans-serif");
    ctx.set_fill_style_str(&muted);
    ctx.set_stroke_style_str("rgba(128,128,128,.18)");
    ctx.set_line_width(1.0);
    // Pressure decades (and a mid step when the range is narrow).
    let mut lg = lg_lo.ceil();
    while lg <= lg_hi {
        let y = sy(10f64.powf(lg));
        let _ = ctx.begin_path();
        let _ = ctx.move_to(x0, y);
        let _ = ctx.line_to(x1, y);
        let _ = ctx.stroke();
        let _ = ctx.fill_text(&format!("{} kPa", format_pow10(lg)), 6.0, y + 4.0);
        lg += 1.0;
    }
    // Enthalpy ticks.
    let h_span = h_max - h_min;
    let h_step = nice_step(h_span / 6.0);
    let mut h_tick = (h_min / h_step).ceil() * h_step;
    while h_tick < h_max {
        let x = sx(h_tick);
        let _ = ctx.begin_path();
        let _ = ctx.move_to(x, y0);
        let _ = ctx.line_to(x, y1);
        let _ = ctx.stroke();
        let _ = ctx.fill_text(&format!("{h_tick:.0}"), x - 12.0, y1 + 18.0);
        h_tick += h_step;
    }
    let _ = ctx.fill_text("specific enthalpy h [kJ/kg]", (x0 + x1) / 2.0 - 70.0, y1 + 34.0);

    // --- Cycle polyline ------------------------------------------------------
    let pt = |i: usize| (sx(r.points[i].state.h), sy(r.points[i].state.p_kpa));
    let (x4, y4) = pt(4);
    let (x1p, y1p) = pt(0);
    let (x2, y2) = pt(2);
    let (x3, y3) = pt(3);
    let (x2s, y2s) = pt(1);

    ctx.set_stroke_style_str(&accent);
    ctx.set_line_width(2.2);
    ctx.begin_path();
    let _ = ctx.move_to(x4, y4);
    let _ = ctx.line_to(x1p, y1p); // evaporator isobar
    let _ = ctx.line_to(x2, y2); // compression
    let _ = ctx.line_to(x3, y3); // condenser isobar
    let _ = ctx.line_to(x4, y4); // throttling (isenthalpic → vertical)
    let _ = ctx.stroke();

    // Isentropic reference 1→2s, dashed.
    ctx.set_line_width(1.4);
    ctx.set_stroke_style_str(&muted);
    ctx.begin_path();
    let _ = ctx.move_to(x1p, y1p);
    let _ = ctx.line_to(x2s, y2s);
    let _ = ctx.stroke();

    // Isobar guide lines extending to the dome.
    ctx.set_line_width(1.0);
    ctx.begin_path();
    let _ = ctx.move_to(x0, y1p);
    let _ = ctx.line_to(x1p, y1p);
    let _ = ctx.move_to(x0, y2);
    let _ = ctx.line_to(x3, y2);
    let _ = ctx.stroke();

    // --- Point markers + labels --------------------------------------------
    ctx.set_font("bold 13px system-ui, sans-serif");
    let markers = [
        (x1p, y1p, "1"),
        (x2, y2, "2"),
        (x3, y3, "3"),
        (x4, y4, "4"),
    ];
    for (x, y, name) in markers {
        ctx.begin_path();
        let _ = ctx.arc(x, y, 4.0, 0.0, std::f64::consts::TAU);
        ctx.set_fill_style_str(&accent);
        let _ = ctx.fill();
        let _ = ctx.fill_text(name, x + 7.0, y - 7.0);
    }
    // 2s hollow marker.
    ctx.begin_path();
    let _ = ctx.arc(x2s, y2s, 3.5, 0.0, std::f64::consts::TAU);
    ctx.set_stroke_style_str(&muted);
    let _ = ctx.stroke();
    let _ = ctx.fill_text("2s", x2s + 6.0, y2s + 14.0);

    // COP annotation.
    ctx.set_fill_style_str(&fg);
    ctx.set_font("bold 14px system-ui, sans-serif");
    let _ = ctx.fill_text(
        &format!(
            "COP {} · q {} kJ/kg · w {} kJ/kg",
            fmt_num(r.cop, 2),
            fmt_num(r.q_evap_kjkg, 1),
            fmt_num(r.w_comp_kjkg, 1)
        ),
        x0 + 8.0,
        y0 + 18.0,
    );
}

fn fmt_num(v: f64, digits: usize) -> String {
    format!("{v:.digits$}")
}

/// Rounds an axis step to 1/2/5×10ⁿ.
fn nice_step(raw: f64) -> f64 {
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let nice = if norm < 1.5 {
        1.0
    } else if norm < 3.5 {
        2.0
    } else if norm < 7.5 {
        5.0
    } else {
        10.0
    };
    nice * mag
}

/// Compact axis label for a log10 exponent (plain-text 1e-n form).
fn format_pow10(lg: f64) -> String {
    let n = lg.round() as i64;
    if n == 0 {
        "1".to_string()
    } else {
        format!("1e{n}")
    }
}
