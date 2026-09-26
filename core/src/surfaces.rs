//! Carving surface outlines, in mm, with the top-left of their box at (0, 0).
use crate::geometry::{pt, Point};
use std::f64::consts::PI;

/// A plaque like a routed craft board: notched (concave) corners and a
/// gently arched top and bottom edge.
pub fn plaque(w: f64, h: f64) -> Vec<Point> {
    let r = 0.13 * w.min(h); let bulge = 0.035 * h;
    let mut out = vec![];
    // corners are quarter circles centred on the box corners, bowing inward
    let corner = |c: Point, a0: f64, out: &mut Vec<Point>| for k in 0..=12 { let a = a0 - k as f64 / 12.0 * PI / 2.0; out.push(pt(c.x + r * a.cos(), c.y + r * a.sin())); };
    let arch = |a: Point, b: Point, up: f64, out: &mut Vec<Point>| for k in 1..24 { let t = k as f64 / 24.0; out.push(pt(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t - up * (PI * t).sin())); };
    corner(pt(0.0, 0.0), PI / 2.0, &mut out);            // top-left: (0, r) round to (r, 0)
    arch(pt(r, 0.0), pt(w - r, 0.0), bulge, &mut out);
    corner(pt(w, 0.0), PI, &mut out);                     // top-right: (w - r, 0) round to (w, r)
    corner(pt(w, h), 3.0 * PI / 2.0, &mut out);           // bottom-right: (w, h - r) round to (w - r, h)
    arch(pt(w - r, h), pt(r, h), -bulge, &mut out);
    corner(pt(0.0, h), 0.0, &mut out);                    // bottom-left: (r, h) round to (0, h - r)
    out
}
pub fn oval(w: f64, h: f64) -> Vec<Point> { (0..96).map(|k| { let a = k as f64 / 96.0 * 2.0 * PI; pt(w / 2.0 + w / 2.0 * a.cos(), h / 2.0 + h / 2.0 * a.sin()) }).collect() }
pub fn rectangle(w: f64, h: f64) -> Vec<Point> { vec![pt(0.0, 0.0), pt(w, 0.0), pt(w, h), pt(0.0, h)] }
