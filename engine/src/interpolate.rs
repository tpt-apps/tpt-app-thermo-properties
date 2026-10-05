//! Small interpolation helpers over per-fluid anchor tables.
//!
//! Every correlation in this engine is a closed-form corresponding-states
//! equation *shaped* by a handful of per-fluid anchor points taken from
//! published reference tables. The anchors are data (see [`crate::fluids`]),
//! not hand-fitted coefficients — so any anchor can be cross-checked against
//! an independent literature value in `tests/validation.rs`.

/// Evaluates the quadratic through three `(x, y)` anchors at `x`.
///
/// With fewer than three anchors this degenerates gracefully: one anchor is
/// constant, two anchors are linear. Extrapolates outside the anchor range —
/// callers keep evaluation ranges inside the correlation's stated validity.
pub fn eval_curve(anchors: &[(f64, f64)], x: f64) -> f64 {
    match anchors.len() {
        0 => f64::NAN,
        1 => anchors[0].1,
        2 => {
            let (x0, y0) = anchors[0];
            let (x1, y1) = anchors[1];
            y0 + (y1 - y0) * (x - x0) / (x1 - x0)
        }
        _ => {
            // Lagrange through the first three anchors (all tables carry
            // exactly three; extra anchors would need windowing, which no
            // table here uses).
            let (x0, y0) = anchors[0];
            let (x1, y1) = anchors[1];
            let (x2, y2) = anchors[2];
            let l0 = (x - x1) * (x - x2) / ((x0 - x1) * (x0 - x2));
            let l1 = (x - x0) * (x - x2) / ((x1 - x0) * (x1 - x2));
            let l2 = (x - x0) * (x - x1) / ((x2 - x0) * (x2 - x1));
            y0 * l0 + y1 * l1 + y2 * l2
        }
    }
}

/// Piecewise-linear interpolation with clamped ends: below the first anchor's
/// x the first y is used, above the last anchor's x the last y.
pub fn eval_piecewise(anchors: &[(f64, f64)], x: f64) -> f64 {
    let n = anchors.len();
    match n {
        0 => f64::NAN,
        1 => anchors[0].1,
        _ => {
            if x <= anchors[0].0 {
                return anchors[0].1;
            }
            if x >= anchors[n - 1].0 {
                return anchors[n - 1].1;
            }
            for window in anchors.windows(2) {
                let (x0, y0) = window[0];
                let (x1, y1) = window[1];
                if x <= x1 {
                    return y0 + (y1 - y0) * (x - x0) / (x1 - x0);
                }
            }
            anchors[n - 1].1
        }
    }
}

/// Integrates the quadratic through `anchors` from `a` to `x` (analytically —
/// the Lagrange form is a polynomial, so no numeric quadrature needed).
pub fn integrate_curve(anchors: &[(f64, f64)], a: f64, x: f64) -> f64 {
    // Expand the Lagrange quadratic into monomial coefficients c0 + c1*T + c2*T².
    let (c0, c1, c2) = curve_coeffs(anchors);
    // ∫(c0 + c1·T + c2·T²) dT from a to x
    let f = |t: f64| c0 * t + c1 * t * t / 2.0 + c2 * t * t * t / 3.0;
    f(x) - f(a)
}

/// Integrates curve/T from `a` to `x`: ∫(c0/T + c1 + c2·T) dT.
pub fn integrate_curve_over_t(anchors: &[(f64, f64)], a: f64, x: f64) -> f64 {
    let (c0, c1, c2) = curve_coeffs(anchors);
    let f = |t: f64| c0 * t.abs().max(1e-12).ln() + c1 * t + c2 * t * t / 2.0;
    f(x) - f(a)
}

/// Monomial coefficients (c0, c1, c2) of the quadratic through `anchors`.
fn curve_coeffs(anchors: &[(f64, f64)]) -> (f64, f64, f64) {
    match anchors.len() {
        0 => (f64::NAN, 0.0, 0.0),
        1 => (anchors[0].1, 0.0, 0.0),
        2 => {
            let (x0, y0) = anchors[0];
            let (x1, y1) = anchors[1];
            let c1 = (y1 - y0) / (x1 - x0);
            (y0 - c1 * x0, c1, 0.0)
        }
        _ => {
            let (x0, y0) = anchors[0];
            let (x1, y1) = anchors[1];
            let (x2, y2) = anchors[2];
            // Solve the 3x3 Vandermonde system by elimination.
            let denom = (x0 - x1) * (x0 - x2) * (x1 - x2);
            let c2 = (x1 * (y0 - y2) + x2 * (y1 - y0) + x0 * (y2 - y1)) / denom;
            let c1 = (y1 - y0) / (x1 - x0) - c2 * (x0 + x1);
            let c0 = y0 - c1 * x0 - c2 * x0 * x0;
            (c0, c1, c2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quadratic_through_anchors_hits_them() {
        let a = [(0.0, 1.336), (25.0, 1.425), (50.0, 1.541)];
        assert!((eval_curve(&a, 0.0) - 1.336).abs() < 1e-12);
        assert!((eval_curve(&a, 25.0) - 1.425).abs() < 1e-12);
        assert!((eval_curve(&a, 50.0) - 1.541).abs() < 1e-12);
    }

    #[test]
    fn integration_is_consistent_with_evaluation() {
        // d/dx of integrate_curve == eval_curve at any interior point.
        let a = [(250.0, 0.812), (300.0, 0.852), (400.0, 0.955)];
        let x = 330.0;
        let eps = 1e-4;
        let slope = (integrate_curve(&a, 250.0, x + eps) - integrate_curve(&a, 250.0, x - eps)) / (2.0 * eps);
        assert!((slope - eval_curve(&a, x)).abs() < 1e-6);
    }

    #[test]
    fn piecewise_clamps_outside() {
        let a = [(0.0, 10.0), (10.0, 20.0), (20.0, 40.0)];
        assert_eq!(eval_piecewise(&a, -5.0), 10.0);
        assert_eq!(eval_piecewise(&a, 25.0), 40.0);
        assert!((eval_piecewise(&a, 15.0) - 30.0).abs() < 1e-12);
    }
}
