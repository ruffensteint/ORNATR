//! The Rococo workspace: designs from the rococo reference library
//! (`scroll_core::rococo`) edited on the page. Drag a rim's points to reshape
//! it and the ornament pinned to it follows; drag ornament along its rim;
//! select any element to change it; add rims, fronds, shells, rosettes,
//! frond runs, crests and pockets; export the SVG at actual size.
use super::*;
use crate::platform;
use crate::shelf::{self as sh, bevel_button, big_tab, caption, divider, seg, shelf_caption, tile_with};
use scroll_core::rocaille::{self, Part, Turnover};
use scroll_core::rococo::{element_parts, remove_node, set_handle, split_at, structure, Anchor, Design, Element, Kind, Spine, STRUCTURES};
use std::collections::HashMap;
use std::f64::consts::PI;
use web_time::Instant;

const AUTOSAVE: &str = "rococo-current.json";
/// The first design: the loop cartouche on a 240 × 250 mm page.
const START: (&str, f64, f64) = ("cartouche", 240.0, 250.0);

/// A handle on the selected element: a rim point or one of its two handles
/// (in, out), a pocket point, a frond's or
/// shell's aim (direction and size), or either end of a run or crest.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Handle { Node(usize), In(usize), Out(usize), Twist(usize), Aim, From, To }

enum RDrag { Pan, Handle { id: u32, h: Handle }, Body { id: u32, start: Point } }

/// One element as last built: what it was built from (itself and its rim) and its parts.
struct Built { key: (Element, Option<Kind>), parts: Vec<Part>, bounds: Bounds }

pub struct RococoState {
    pub design: Design,
    /// The library structure a new design starts from.
    pub structure: String,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    past: Vec<Design>,
    future: Vec<Design>,
    /// The last edit's key and time, so dragging a slider is one undo step.
    last_edit: Option<(String, Instant)>,
    pub selected: Option<u32>,
    handle: Option<Handle>,
    pub filled: bool,
    pub show_grid: bool,
    drag: Option<RDrag>,
    drag_before: Option<Design>,
    /// Rim spines of the design the drag started from.
    drag_spines: HashMap<u32, Spine>,
    // cached geometry for the current design
    cached_for: Option<Design>,
    built: HashMap<u32, Built>,
    spines: HashMap<u32, Spine>,
    drawing: Option<rocaille::Drawing>,
    /// The filled preview, rasterized at this many pixels per mm.
    texture: Option<(egui::TextureHandle, f64)>,
    texture_stale: bool,
    autosaved: Option<String>,
    pub message: String,
    zoom: f32,
    origin: Pos2,
    fitted: bool,
    pub cursor_mm: Option<Point>,
    /// Library tiles: each structure's rim spines and shell hinges, in a unit box.
    thumbs: Option<Vec<(Vec<Vec<Point>>, Vec<Point>)>>,
}

impl RococoState {
    pub fn new() -> RococoState {
        let text = platform::store_read(AUTOSAVE);
        let design = text.as_deref().and_then(|t| io::parse_rococo(t).ok()).unwrap_or_else(|| Design::from_structure(START.0, START.1, START.2).unwrap());
        RococoState { design, structure: START.0.into(), path: None, dirty: false, past: vec![], future: vec![], last_edit: None, selected: None, handle: None, filled: true, show_grid: false,
            drag: None, drag_before: None, drag_spines: HashMap::new(), cached_for: None, built: HashMap::new(), spines: HashMap::new(), drawing: None, texture: None, texture_stale: true,
            autosaved: text, message: String::new(), zoom: 3.0, origin: Pos2::ZERO, fitted: false, cursor_mm: None, thumbs: None }
    }

    // ---------- edits and undo ----------
    fn change(&mut self, next: Design) { self.change_keyed(None, next); }
    /// Replace the design; edits with the same key in quick succession (a
    /// slider being dragged) make one undo step.
    fn change_keyed(&mut self, key: Option<String>, next: Design) {
        if next == self.design { return; }
        let now = Instant::now();
        let merge = key.is_some() && self.last_edit.as_ref().is_some_and(|(k, t)| Some(k) == key.as_ref() && now.duration_since(*t).as_secs_f32() < 1.0);
        if merge { self.design = next; } else {
            self.past.push(std::mem::replace(&mut self.design, next));
            if self.past.len() > 80 { self.past.remove(0); }
        }
        self.future.clear(); self.dirty = true;
        self.last_edit = key.map(|k| (k, now));
    }
    pub fn undo(&mut self) { if let Some(p) = self.past.pop() { self.future.push(std::mem::replace(&mut self.design, p)); self.after_history(); } }
    pub fn redo(&mut self) { if let Some(n) = self.future.pop() { self.past.push(std::mem::replace(&mut self.design, n)); self.after_history(); } }
    fn after_history(&mut self) { self.dirty = true; self.last_edit = None; if self.selected.is_some_and(|id| self.design.get(id).is_none()) { self.selected = None; self.handle = None; } }
    pub fn can_undo(&self) -> bool { !self.past.is_empty() }
    pub fn can_redo(&self) -> bool { !self.future.is_empty() }
    fn edit(&mut self, id: u32, kind: Kind) {
        let mut d = self.design.clone();
        if let Some(e) = d.get_mut(id) { e.kind = kind; }
        self.change_keyed(Some(format!("edit-{id}")), d);
    }

    // ---------- cache ----------
    fn refresh(&mut self) {
        if self.cached_for.as_ref() == Some(&self.design) { return; }
        self.spines = self.design.spines();
        let mut built = HashMap::new();
        for e in &self.design.elements {
            let rim = e.rim().and_then(|r| self.design.get(r)).map(|r| r.kind.clone());
            let key = (e.clone(), rim);
            let b = match self.built.remove(&e.id) {
                Some(b) if b.key == key => b,
                _ => {
                    let parts = element_parts(e, &self.spines).unwrap_or_default();
                    let bounds = Bounds::of(&parts.iter().flat_map(|p| p.outline.iter().copied()).collect::<Vec<_>>());
                    Built { key, parts, bounds }
                }
            };
            built.insert(e.id, b);
        }
        self.built = built;
        // ornament grown from its rim is drawn as one surface with it
        let parts: Vec<(u32, Vec<Part>)> = self.design.elements.iter().map(|e| (e.id, self.built[&e.id].parts.clone())).collect();
        self.drawing = Some(rocaille::draw(&self.design.layered(&parts)));
        self.cached_for = Some(self.design.clone());
        self.texture_stale = true;
        if self.selected.is_some_and(|id| self.design.get(id).is_none()) { self.selected = None; self.handle = None; }
    }
    /// Keep the current design on this computer.
    fn autosave(&mut self) {
        if self.drag.is_some() { return; }
        let text = io::save_rococo(&self.design);
        if self.autosaved.as_ref() == Some(&text) { return; }
        let _ = platform::store_write(AUTOSAVE, &text);
        self.autosaved = Some(text);
    }

    // ---------- files ----------
    pub fn title(&self) -> String {
        let name = self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Rococo design".into());
        format!("{name}{} — ORNATR", if self.dirty { " •" } else { "" })
    }
    /// A new design on a `w` × `h` page, from the last library structure chosen.
    pub fn new_design(&mut self, w: f64, h: f64) {
        let d = Design::from_structure(&self.structure, w, h).unwrap_or_else(|| Design::empty(w, h));
        self.change(d); self.path = None; self.dirty = false; self.selected = None; self.handle = None; self.fitted = false;
        self.message = format!("New {} × {} mm design. Undo returns to the previous one.", w, h);
    }
    /// Start again from a library structure, fitted to the page.
    pub fn use_structure(&mut self, id: &str) {
        let Some(d) = Design::from_structure(id, self.design.width, self.design.height) else { return };
        self.structure = id.into();
        self.change(d); self.selected = None; self.handle = None;
        let name = STRUCTURES.iter().find(|s| s.0 == id).map_or(id, |s| s.1);
        self.message = format!("{name} fitted to the page. Drag rim points to reshape it; Undo returns to the previous design.");
    }
    /// A picked file, once read (see `App::opened`, which sends rococo designs here).
    pub fn open_text(&mut self, picked: platform::Opened) {
        let (path, text) = match picked { Ok(p) => p, Err(e) => { self.message = format!("Could not open this file. {e}"); return } };
        match io::parse_rococo(&text) {
            Ok(d) => { self.change(d); self.path = Some(path); self.dirty = false; self.selected = None; self.handle = None; self.fitted = false; self.message = "Rococo design opened.".into(); }
            // scroll layouts share the .ornatr extension: say where a file belongs
            Err(_) if io::parse(&text).is_ok() => self.message = "That is a scroll layout. Switch to the Scroll workspace and open it there.".into(),
            Err(_) if io::parse_chip(&text).is_ok() || io::parse_box(&text).is_ok() => self.message = "That is a chip layout. Switch to the Chip workspace and open it there.".into(),
            Err(e) => self.message = format!("Could not open this design. {e}"),
        }
    }
    pub fn save(&mut self, choose: bool) {
        let path = if choose || self.path.is_none() {
            match platform::choose_save("ORNATR rococo design", &["ornatr"], "rococo-design.ornatr") { Some(p) => p, None => return }
        } else { self.path.clone().unwrap() };
        match platform::write_file(&path, &io::save_rococo(&self.design)) { Ok(()) => { self.path = Some(path); self.dirty = false; self.message = "Rococo design saved.".into(); } Err(e) => self.message = format!("Could not save: {e}") }
    }
    pub fn export(&mut self) {
        if let Some(p) = platform::choose_save("SVG", &["svg"], "rococo-pattern.svg") {
            match platform::write_file(&p, &self.design.svg()) { Ok(()) => self.message = format!("Exported {}.", platform::shown(&p)), Err(e) => self.message = format!("Could not export: {e}") }
        }
    }

    // ---------- selection ----------
    /// The rim the selection belongs to: the selected rim, or the rim the selected ornament hangs on.
    fn current_rim(&self) -> Option<u32> {
        let e = self.design.get(self.selected?)?;
        if e.is_rim() { Some(e.id) } else { e.rim().filter(|r| self.design.get(*r).is_some()) }
    }
    /// Delete the selected rim point (when one is picked and the rim keeps
    /// three), else the selected element (a rim with its ornament).
    pub fn remove_selected(&mut self) {
        let Some(id) = self.selected else { return };
        let mut d = self.design.clone();
        if let (Some(Handle::Twist(i)), Some(Element { kind: Kind::Rim { twists, .. }, .. })) = (self.handle, d.get_mut(id)) {
            if i < twists.len() { twists.remove(i); self.handle = None; self.change(d); self.message = "Turn-over removed.".into(); return; }
        }
        if let (Some(Handle::Node(k)), Some(e)) = (self.handle, d.get_mut(id)) {
            match &mut e.kind {
                Kind::Rim { nodes, .. } if nodes.len() > 2 => { remove_node(nodes, k); self.handle = None; self.change(d); self.message = "Point removed.".into(); return; }
                Kind::Pocket { ctrl, .. } | Kind::Trellis { ctrl, .. } if ctrl.len() > 4 => { ctrl.remove(k.min(ctrl.len() - 1)); self.handle = None; self.change(d); self.message = "Point removed.".into(); return; }
                _ => {}
            }
        }
        let label = d.get(id).map_or("Element", |e| e.label());
        let n = d.remove(id);
        self.change(d); self.selected = None; self.handle = None;
        self.message = if n > 1 { format!("{label} removed with the {} pieces of ornament on it. Undo brings them back.", n - 1) } else { format!("{label} removed. Undo brings it back.") };
    }
    pub fn restack(&mut self, front: bool) {
        let Some(id) = self.selected else { return };
        let mut d = self.design.clone(); d.restack(id, front); self.change(d);
    }
    /// Add an element of `what` (on the current rim where it needs one) and select it.
    fn add(&mut self, what: &str) {
        let mut d = self.design.clone();
        let c = pt(d.width / 2.0, d.height / 2.0); let m = d.width.min(d.height);
        let rim = self.current_rim();
        let id = match (what, rim) {
            ("rim", _) => Some(d.add_rim(c, m * 0.5)),
            ("pocket", _) => Some(d.add_pocket(c, m * 0.15)),
            ("trellis", _) => Some(d.add_trellis(c, m * 0.15)),
            ("cabochon", Some(r)) => d.add_cabochon(r, 0.5),
            ("frond", Some(r)) => d.add_frond(r, 0.5),
            ("shell", Some(r)) => d.add_shell(r, 0.5),
            ("rosette", Some(r)) => d.add_rosette(r, 0.5),
            ("run", Some(r)) => d.add_run(r, 0.25, 0.75),
            ("crest", Some(r)) => d.add_frill(r, 0.3, 0.7),
            _ => { self.message = "Select a rim first (or something on it): new ornament goes on that rim.".into(); return; }
        };
        let Some(id) = id else { return };
        let label = d.get(id).map_or("Element", |e| e.label());
        self.message = match what { "rim" => "Rim added in the middle of the page. Drag its points and handles to shape it; Alt-click it to add a point.".into(), "pocket" => "Pocket added behind everything. Drag its points to shape it.".into(), "trellis" => "Trellis field added behind everything. Drag its points to shape it; its cell size and florets are under Selection.".into(), _ => format!("{label} added halfway along the rim. Drag it along the rim, or drag its handle to aim it.") };
        self.change(d); self.selected = Some(id); self.handle = None;
    }

    // ---------- view ----------
    fn to_screen(&self, p: Point) -> Pos2 { Pos2::new(self.origin.x + p.x as f32 * self.zoom, self.origin.y + p.y as f32 * self.zoom) }
    fn to_mm(&self, s: Pos2) -> Point { pt(((s.x - self.origin.x) / self.zoom) as f64, ((s.y - self.origin.y) / self.zoom) as f64) }
    fn fit(&mut self, rect: Rect) {
        let (w, h) = (self.design.width as f32, self.design.height as f32);
        self.zoom = ((rect.width() - 80.0) / w).min((rect.height() - 80.0) / h).max(0.2);
        self.origin = Pos2::new(rect.center().x - w * self.zoom / 2.0, rect.center().y - h * self.zoom / 2.0);
        self.fitted = true;
    }
    pub fn fit_page(&mut self) { self.fitted = false; }

    /// The topmost element under `mm`.
    fn hit(&self, mm: Point) -> Option<u32> {
        self.design.elements.iter().rev().find(|e| self.built.get(&e.id).is_some_and(|b| b.bounds.contains(mm) && b.parts.iter().any(|p| inside(mm, &p.outline)))).map(|e| e.id)
    }
    /// The topmost rim under `mm` (ornament over it doesn't count).
    fn hit_rim(&self, mm: Point) -> Option<u32> {
        self.design.elements.iter().rev().filter(|e| e.is_rim()).find(|e| self.built.get(&e.id).is_some_and(|b| b.bounds.contains(mm) && b.parts.iter().any(|p| inside(mm, &p.outline)))).map(|e| e.id)
    }
    /// Handles on the selected element, in mm.
    fn handles(&self) -> Vec<(Handle, Point)> {
        let Some(e) = self.selected.and_then(|id| self.design.get(id)) else { return vec![] };
        let place = |at: &Anchor| Design::place(&self.spines, at);
        let aim = |base: Option<(Point, f64)>, a: f64, l: f64| base.map(|(p, h)| vec![(Handle::Aim, pt(p.x + (h + a).cos() * l, p.y + (h + a).sin() * l))]).unwrap_or_default();
        match &e.kind {
            Kind::Rim { nodes, .. } => {
                // the handles at the free ends do nothing, so they aren't shown
                let last = nodes.len().saturating_sub(1);
                let mut v: Vec<(Handle, Point)> = nodes.iter().enumerate().flat_map(|(k, n)| { let mut v = vec![(Handle::Node(k), n.p)]; if k > 0 { v.push((Handle::In(k), n.a)); } if k < last { v.push((Handle::Out(k), n.b)); } v }).collect();
                // where it turns over: a handle on the rim, dragged along it
                if let (Kind::Rim { twists, .. }, Some(s)) = (&e.kind, self.spines.get(&e.id)) { v.extend(twists.iter().enumerate().map(|(i, t)| (Handle::Twist(i), s.frame(*t).0))); }
                v
            }
            Kind::Pocket { ctrl, .. } | Kind::Trellis { ctrl, .. } => ctrl.iter().enumerate().map(|(k, p)| (Handle::Node(k), *p)).collect(),
            Kind::Cabochon { at, turn, rx, .. } => aim(place(at), *turn, *rx),
            Kind::Frond { at, heading, length, .. } => aim(place(at), *heading, *length),
            Kind::Shell { at, axis, r, .. } => aim(place(at), *axis, *r),
            Kind::Run { rim, from, to, .. } | Kind::Frill { rim, from, to, .. } => match self.spines.get(rim) { Some(s) => vec![(Handle::From, s.frame(*from).0), (Handle::To, s.frame(*to).0)], None => vec![] },
            Kind::Rosette { .. } => vec![],
        }
    }
    /// Apply the drag in progress with the pointer at `mm` (from the design the drag started on).
    fn drag_to(&mut self, mm: Point) {
        let Some(before) = self.drag_before.as_ref() else { return };
        let mut d = before.clone();
        let sp = &self.drag_spines;
        let share_on = |rim: u32, p: Point| sp.get(&rim).map(|s| s.share_of(s.nearest(p).0));
        match self.drag {
            Some(RDrag::Handle { id, h }) => {
                let Some(e) = d.get_mut(id) else { return };
                match (&mut e.kind, h) {
                    // a rim point carries its handles with it; a handle swings its partner round to stay in line
                    (Kind::Rim { nodes, .. }, Handle::Node(k)) => if let Some(n) = nodes.get_mut(k) { let (dx, dy) = (mm.x - n.p.x, mm.y - n.p.y); for q in [&mut n.p, &mut n.a, &mut n.b] { q.x += dx; q.y += dy; } },
                    (Kind::Rim { nodes, .. }, Handle::In(k) | Handle::Out(k)) => set_handle(nodes, k, matches!(h, Handle::Out(_)), mm),
                    (Kind::Pocket { ctrl, .. } | Kind::Trellis { ctrl, .. }, Handle::Node(k)) => if let Some(p) = ctrl.get_mut(k) { *p = mm; },
                    (Kind::Rim { twists, .. }, Handle::Twist(i)) => if let (Some(t), Some(u)) = (twists.get_mut(i), share_on(id, mm)) { *t = u.clamp(0.0, 1.0); },
                    (Kind::Cabochon { at, turn, rx, ry }, Handle::Aim) => if let Some((p, base)) = Design::place(sp, at) {
                        // angle and size; the oval keeps its proportions
                        let l = (mm.x - p.x).hypot(mm.y - p.y).max(2.0); *ry *= l / rx.max(1e-9); *rx = l; *turn = (mm.y - p.y).atan2(mm.x - p.x) - base;
                    },
                    (Kind::Frond { at, heading, length, .. }, Handle::Aim) => if let Some((p, base)) = Design::place(sp, at) {
                        *heading = (mm.y - p.y).atan2(mm.x - p.x) - base; *length = (mm.x - p.x).hypot(mm.y - p.y).max(5.0);
                    },
                    (Kind::Shell { at, axis, r, .. }, Handle::Aim) => if let Some((p, base)) = Design::place(sp, at) {
                        *axis = (mm.y - p.y).atan2(mm.x - p.x) - base; *r = (mm.x - p.x).hypot(mm.y - p.y).max(3.0);
                    },
                    (Kind::Run { rim, from, to, .. } | Kind::Frill { rim, from, to, .. }, Handle::From | Handle::To) => if let Some(u) = share_on(*rim, mm) {
                        if h == Handle::From { *from = u; } else { *to = u; }
                    },
                    _ => {}
                }
            }
            Some(RDrag::Body { id, start }) => {
                let (dx, dy) = (mm.x - start.x, mm.y - start.y);
                let Some(e) = d.get_mut(id) else { return };
                // ornament on a rim slides along it, keeping its place across
                // the rim, as leaves slide along a backbone (from where it was
                // when the drag started, so it doesn't jump)
                let slide = |at: &mut Anchor| match at {
                    Anchor::Free(p) => *p = pt(p.x + dx, p.y + dy),
                    Anchor::Rim { rim, u, .. } => if let (Some(a), Some(b)) = (share_on(*rim, start), share_on(*rim, mm)) { *u += b - a; },
                };
                match &mut e.kind {
                    Kind::Rim { nodes, .. } => for n in nodes.iter_mut() { for q in [&mut n.p, &mut n.a, &mut n.b] { q.x += dx; q.y += dy; } },
                    Kind::Pocket { ctrl, .. } | Kind::Trellis { ctrl, .. } => for p in ctrl.iter_mut() { *p = pt(p.x + dx, p.y + dy); },
                    Kind::Frond { at, .. } | Kind::Shell { at, .. } | Kind::Rosette { at, .. } | Kind::Cabochon { at, .. } => slide(at),
                    Kind::Run { rim, from, to, .. } | Kind::Frill { rim, from, to, .. } => if let (Some(a), Some(b)) = (share_on(*rim, start), share_on(*rim, mm)) { *from += b - a; *to += b - a; },
                }
            }
            _ => return,
        }
        self.design = d;
    }
    /// Add a point to rim `id` where it was Alt-clicked (the rim keeps its shape).
    fn insert_point(&mut self, id: u32, mm: Point) {
        let mut d = self.design.clone();
        if let Some(Element { kind: Kind::Rim { nodes, .. }, .. }) = d.get_mut(id) {
            let Some(k) = split_at(nodes, mm) else { return };
            self.selected = Some(id); self.handle = Some(Handle::Node(k));
        }
        self.change(d); self.message = "Point added. Drag it to reshape the rim; Delete removes it.".into();
    }

    /// The filled preview: parts in wood back to front, pockets dark, turned-over undersides darker.
    fn raster(&self, k: f64) -> egui::ColorImage {
        let (w, h) = (((self.design.width * k).ceil() as usize).max(1), ((self.design.height * k).ceil() as usize).max(1));
        let mut px = vec![Color32::TRANSPARENT; w * h];
        for e in &self.design.elements {
            if let Some(b) = self.built.get(&e.id) { for p in &b.parts { fill(&mut px, w, h, &p.outline, k, if p.pocket { POCKET } else { WOOD }); } }
        }
        if let Some(d) = &self.drawing { for u in &d.undersides { fill(&mut px, w, h, u, k, UNDERSIDE); } }
        egui::ColorImage { size: [w, h], pixels: px }
    }
    fn thumbs(&mut self) -> &Vec<(Vec<Vec<Point>>, Vec<Point>)> {
        self.thumbs.get_or_insert_with(|| STRUCTURES.iter().map(|(id, _, _)| {
            let d = structure(id).unwrap();
            let spines = d.spines();
            let lines: Vec<Vec<Point>> = d.elements.iter().filter_map(|e| spines.get(&e.id)).map(|s| s.line().into_iter().step_by(6).collect()).collect();
            let shells: Vec<Point> = d.elements.iter().filter_map(|e| if let Kind::Shell { at, r, .. } = &e.kind { (*r > 25.0).then(|| Design::place(&spines, at).map(|q| q.0)).flatten() } else { None }).collect();
            let all: Vec<Point> = lines.iter().flatten().copied().collect();
            let b = Bounds::of(&all); let s = (b.r - b.l).max(b.b - b.t).max(1e-6);
            let (ox, oy) = (b.l + ((b.r - b.l) - s) / 2.0, b.t + ((b.b - b.t) - s) / 2.0);
            let norm = |p: &Point| pt((p.x - ox) / s, (p.y - oy) / s);
            (lines.iter().map(|l| l.iter().map(norm).collect()).collect(), shells.iter().map(norm).collect())
        }).collect())
    }
}

const WOOD: Color32 = Color32::from_rgb(201, 167, 107);
const UNDERSIDE: Color32 = Color32::from_rgb(168, 134, 77);
const POCKET: Color32 = Color32::from_rgb(90, 90, 90);
const CARVED_INK: Color32 = Color32::from_rgb(107, 79, 34);

/// Fill a polygon (even-odd) into a `w` × `h` image at `k` pixels per mm.
fn fill(px: &mut [Color32], w: usize, h: usize, poly: &[Point], k: f64, col: Color32) {
    let n = poly.len(); if n < 3 { return; }
    // edges as (top y, bottom y, x at top, dx/dy), sorted by their top
    let mut edges: Vec<(f64, f64, f64, f64)> = (0..n).filter_map(|i| {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let (ax, ay, bx, by) = (a.x * k, a.y * k, b.x * k, b.y * k);
        if (ay - by).abs() < 1e-12 { return None; }
        let (x0, y0, x1, y1) = if ay < by { (ax, ay, bx, by) } else { (bx, by, ax, ay) };
        Some((y0, y1, x0, (x1 - x0) / (y1 - y0)))
    }).collect();
    if edges.is_empty() { return; }
    edges.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let ymax = edges.iter().map(|e| e.1).fold(f64::MIN, f64::max);
    let r0 = (edges[0].0 - 0.5).ceil().max(0.0) as usize;
    let r1 = ((ymax - 0.5).floor()).min(h as f64 - 1.0);
    if r1 < r0 as f64 { return; }
    let (mut next, mut active, mut xs): (usize, Vec<usize>, Vec<f64>) = (0, vec![], vec![]);
    for row in r0..=r1 as usize {
        let yc = row as f64 + 0.5;
        while next < edges.len() && edges[next].0 <= yc { active.push(next); next += 1; }
        active.retain(|&i| edges[i].1 > yc);
        xs.clear();
        xs.extend(active.iter().map(|&i| { let e = edges[i]; e.2 + (yc - e.0) * e.3 }));
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pair in xs.chunks_exact(2) {
            let a = (pair[0] - 0.5).ceil().max(0.0); let b = (pair[1] - 0.5).floor().min(w as f64 - 1.0);
            if b < a { continue; }
            let row_px = &mut px[row * w..row * w + w];
            for p in &mut row_px[a as usize..=b as usize] { *p = col; }
        }
    }
}

fn wrap_angle(a: f64) -> f64 { let mut a = a % (2.0 * PI); if a > PI { a -= 2.0 * PI; } if a <= -PI { a += 2.0 * PI; } a }
/// A slider in degrees over an angle kept in radians; the value only changes when the slider is moved.
fn angle(ui: &mut egui::Ui, v: &mut f64, label: &str) {
    let mut d = wrap_angle(*v).to_degrees();
    if ui.add(egui::Slider::new(&mut d, -180.0..=180.0).text(label).suffix("°").fixed_decimals(0)).changed() { *v = d.to_radians(); }
}
fn mm_slider(ui: &mut egui::Ui, v: &mut f64, range: std::ops::RangeInclusive<f64>, label: &str) {
    ui.add(egui::Slider::new(v, range).text(label).suffix(" mm").fixed_decimals(1).clamping(egui::SliderClamping::Never));
}
fn turnover_choice(ui: &mut egui::Ui, t: &Theme, v: &mut Turnover) {
    segmented(ui, t, &[(Turnover::None, "None"), (Turnover::Roll, "Roll"), (Turnover::Flap, "Flap"), (Turnover::Curl, "Curl")], v);
}

impl App {
    pub(crate) fn rococo_shortcuts(&mut self, ctx: &egui::Context, undo: bool, redo: bool, save: bool, save_as: bool, open: bool, new: bool, typing: bool) {
        if undo { self.rococo.undo(); }
        if redo { self.rococo.redo(); }
        if save_as { self.rococo.save(true); } else if save { self.rococo.save(false); }
        if open { self.open_rococo(ctx); }
        if new { self.open_new_dialog(); }
        if typing { return; }
        let (del, esc) = ctx.input(|i| (i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace), i.key_pressed(egui::Key::Escape)));
        if del { self.rococo.remove_selected(); }
        if esc { self.rococo.selected = None; self.rococo.handle = None; }
    }

    pub(crate) fn rococo_menus(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("File", |ui| {
            if ui.add(egui::Button::new("New rococo design…").shortcut_text("Ctrl+N")).clicked() { self.open_new_dialog(); ui.close_menu(); }
            if ui.add(egui::Button::new("Open rococo design…").shortcut_text("Ctrl+O")).clicked() { ui.close_menu(); self.open_rococo(ui.ctx()); }
            if ui.add(egui::Button::new("Save").shortcut_text("Ctrl+S")).clicked() { ui.close_menu(); self.rococo.save(false); }
            if ui.add(egui::Button::new("Save As…").shortcut_text("Ctrl+Shift+S")).clicked() { ui.close_menu(); self.rococo.save(true); }
            ui.separator();
            if ui.button("Export pattern SVG…").clicked() { ui.close_menu(); self.rococo.export(); }
        });
        ui.menu_button("Edit", |ui| {
            if ui.add_enabled(self.rococo.can_undo(), egui::Button::new("Undo").shortcut_text("Ctrl+Z")).clicked() { self.rococo.undo(); ui.close_menu(); }
            if ui.add_enabled(self.rococo.can_redo(), egui::Button::new("Redo").shortcut_text("Ctrl+Y")).clicked() { self.rococo.redo(); ui.close_menu(); }
            ui.separator();
            let sel = self.rococo.selected.is_some();
            if ui.add_enabled(sel, egui::Button::new("Delete").shortcut_text("Del")).clicked() { self.rococo.remove_selected(); ui.close_menu(); }
            if ui.add_enabled(sel, egui::Button::new("Bring to front")).clicked() { self.rococo.restack(true); ui.close_menu(); }
            if ui.add_enabled(sel, egui::Button::new("Send to back")).clicked() { self.rococo.restack(false); ui.close_menu(); }
        });
        ui.menu_button("View", |ui| {
            if ui.button("Fit page").clicked() { self.rococo.fit_page(); ui.close_menu(); }
            ui.checkbox(&mut self.rococo.show_grid, "Millimetre grid");
            ui.checkbox(&mut self.rococo.filled, "Filled preview").on_hover_text("The carved parts in wood, pockets dark, turned-over undersides darker");
            ui.separator();
            ui.menu_button("Theme", |ui| {
                for id in ThemeId::ALL { if ui.selectable_label(self.prefs.theme == id, id.theme().name).clicked() { self.set_prefs(Prefs { theme: id, ..self.prefs }); ui.close_menu(); } }
            });
            let mut shelf = self.prefs.shelf;
            if ui.checkbox(&mut shelf, "ZBrush-style layout").changed() { self.set_prefs(Prefs { shelf, ..self.prefs }); ui.close_menu(); }
        });
    }

    fn rococo_hint(&self) -> String {
        let st = &self.rococo;
        if !st.message.is_empty() { return st.message.clone(); }
        match st.selected.and_then(|id| st.design.get(id)) {
            None => "Click an element to select it. Pick a starting layout from the library, or add rims and ornament.".into(),
            Some(e) => match e.kind {
                Kind::Rim { .. } => "Rim selected · drag its points to reshape it (its ornament follows) · Alt-click it to add a point · diamonds mark where it turns over".into(),
                Kind::Pocket { .. } => "Pocket selected · drag its points to shape it".into(),
                Kind::Trellis { .. } => "Trellis selected · drag its points to shape the field, or drag it to move it".into(),
                Kind::Run { .. } | Kind::Frill { .. } => format!("{} selected · drag it along its rim, or drag its end handles", e.label()),
                _ => format!("{} selected · drag it along its rim · the handle aims it and sets its size", e.label()),
            },
        }
    }
    pub(crate) fn rococo_status(&mut self, ctx: &egui::Context) {
        let t = self.t();
        let st = &self.rococo;
        let text = format!("{} × {} mm   ·   {} elements", st.design.width, st.design.height, st.design.elements.len());
        let hint = self.rococo_hint();
        egui::TopBottomPanel::bottom("rococo-status").frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::symmetric(14.0, 5.0))).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.visuals_mut().override_text_color = Some(t.dim);
                ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                ui.label(text);
                if let Some(m) = self.rococo.cursor_mm { ui.separator(); ui.label(format!("x {:.1}  y {:.1} mm", m.x, m.y)); }
                ui.separator();
                ui.label(hint);
            });
        });
    }

    /// The classic layout's panel: the library, adding, the selection, the page, the theme.
    pub(crate) fn rococo_side_panel(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::right("rococo-panel").default_width(340.0).min_width(300.0).frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin { left: 16.0, right: 12.0, top: 12.0, bottom: 8.0 })).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(ui.available_width() - 4.0);
                section(ui, t, "Library");
                ui.horizontal_wrapped(|ui| {
                    for (id, name, detail) in STRUCTURES { if ui.selectable_label(self.rococo.structure == id, name).on_hover_text(detail).clicked() { self.rococo.use_structure(id); } }
                });
                section(ui, t, "Add");
                self.rococo_add(ui);
                section(ui, t, "Selected");
                self.rococo_selected(ui);
                section(ui, t, "Page");
                self.rococo_page(ui);
                section(ui, t, "View");
                ui.checkbox(&mut self.rococo.filled, "Filled preview");
                ui.checkbox(&mut self.rococo.show_grid, "Millimetre grid");
                if ui.button("Export pattern SVG…").clicked() { self.rococo.export(); }
                self.theme_tab(ui);
                ui.add_space(12.0);
            });
        });
    }

    fn rococo_add(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let on_rim = self.rococo.current_rim().is_some();
        ui.horizontal_wrapped(|ui| {
            if ui.button("Rim").on_hover_text("A new moulded rim in the middle of the page").clicked() { self.rococo.add("rim"); }
            if ui.button("Pocket").on_hover_text("A pierced hollow, behind everything").clicked() { self.rococo.add("pocket"); }
            for (what, name, tip) in [("frond", "Frond", "A frond on the rim, reaching out from its outer side"), ("shell", "Shell", "A fluted shell on the rim"), ("rosette", "Rosette", "A small rosette set in the rim's channel"), ("run", "Frond run", "Overlapping fronds along the rim's outer roll"), ("crest", "Crest", "A crimped cock's-comb crest along the rim"), ("cabochon", "Cabochon", "A convex oval boss on the rim, as at the crown of a Louis XV crest")] {
                if ui.add_enabled(on_rim, egui::Button::new(name)).on_hover_text(tip).on_disabled_hover_text("Select a rim first").clicked() { self.rococo.add(what); }
            }
        });
        ui.label(egui::RichText::new(if on_rim { "Ornament goes on the selected rim." } else { "Select a rim to add ornament to it." }).small().color(t.dim));
    }

    /// The selected element's settings.
    fn rococo_selected(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let st = &self.rococo;
        let Some(e) = st.selected.and_then(|id| st.design.get(id)).cloned() else {
            ui.label(egui::RichText::new("Click an element on the page to select it. Drag a rim's points to reshape it; everything on it follows.").small().color(t.dim));
            return;
        };
        ui.label(egui::RichText::new(e.label()).strong());
        let mut k = e.kind.clone();
        match &mut k {
            Kind::Rim { nodes, width, outer, hook0, eye0, hook1, eye1, swell, twists } => {
                let mut full = *width * 2.0;
                if ui.add(egui::Slider::new(&mut full, 2.0..=40.0).text("Width").suffix(" mm").fixed_decimals(1)).changed() { *width = full / 2.0; }
                ui.add(egui::Slider::new(swell, 0.0..=1.0).text("Swell").fixed_decimals(2)).on_hover_text("Thin where it starts and ends, full through its body");
                ui.horizontal(|ui| {
                    if ui.button("Add a turn-over").on_hover_text("The rim pinches to its edge and turns over, the moulding crossing to the other side; drag the new handle along the rim").clicked() {
                        // in the widest gap between the ends and the turn-overs already there
                        let mut cuts = vec![0.0]; cuts.extend(twists.iter().copied()); cuts.push(1.0);
                        cuts.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        let at = cuts.windows(2).max_by(|a, b| (a[1] - a[0]).partial_cmp(&(b[1] - b[0])).unwrap()).map_or(0.15, |w| if twists.is_empty() { 0.15 } else { (w[0] + w[1]) / 2.0 });
                        twists.push(at);
                        self.rococo.handle = Some(Handle::Twist(twists.len() - 1));
                    }
                    let picked = matches!(self.rococo.handle, Some(Handle::Twist(_)));
                    if ui.add_enabled(picked, egui::Button::new("Remove turn-over")).on_disabled_hover_text("Click a turn-over handle on the rim first").clicked() { if let Some(Handle::Twist(i)) = self.rococo.handle { if i < twists.len() { twists.remove(i); } self.rococo.handle = None; } }
                });
                if ui.button("Flip the moulding").on_hover_text("Puts the outer roll on the other side of the rim").clicked() { *outer = -*outer; }
                for (name, hook, eye) in [("Start volute", hook0, eye0), ("End volute", hook1, eye1)] {
                    ui.label(name);
                    segmented(ui, t, &[(0.0, "None"), (1.0, "Curls out"), (-1.0, "Curls in")], hook);
                    if *hook != 0.0 { if *eye <= 0.0 { *eye = (*width * 1.6).max(6.0); } mm_slider(ui, eye, 3.0..=60.0, "Volute size"); }
                }
                let can_remove = matches!(self.rococo.handle, Some(Handle::Node(_))) && nodes.len() > 2;
                ui.horizontal(|ui| {
                    if ui.add_enabled(can_remove, egui::Button::new("Remove point")).on_disabled_hover_text("Click one of the rim's points first (a rim keeps at least two)").clicked() { if let Some(Handle::Node(i)) = self.rococo.handle { remove_node(nodes, i); self.rococo.handle = None; } }
                });
                ui.label(egui::RichText::new(format!("{} points · Alt-click the rim to add one", nodes.len())).small().color(t.dim));
            }
            Kind::Frond { heading, length, side, bend, fingers, width, splay, turn, .. } => {
                mm_slider(ui, length, 5.0..=250.0, "Length");
                mm_slider(ui, width, 2.0..=60.0, "Width");
                angle(ui, heading, "Angle");
                ui.add(egui::Slider::new(bend, -1.5..=1.5).text("Bend").fixed_decimals(2));
                ui.add(egui::Slider::new(splay, 0.2..=1.8).text("Splay").fixed_decimals(2)).on_hover_text("How far the fingers fan out near the base");
                ui.add(egui::Slider::new(fingers, 1..=7).text("Fingers"));
                ui.label("Turnover");
                turnover_choice(ui, t, turn);
                if ui.button("Mirror").on_hover_text("Fingers on the other side of the rib").clicked() { *side = -*side; }
            }
            Kind::Shell { axis, span, ribs, r, asym, twist, scallop, .. } => {
                mm_slider(ui, r, 5.0..=200.0, "Size");
                angle(ui, axis, "Angle");
                let mut fan = span.to_degrees();
                if ui.add(egui::Slider::new(&mut fan, 60.0..=200.0).text("Fan").suffix("°").fixed_decimals(0)).changed() { *span = fan.to_radians(); }
                ui.add(egui::Slider::new(ribs, 5..=17).text("Flutes"));
                ui.add(egui::Slider::new(asym, -1.0..=1.0).text("Lean").fixed_decimals(2)).on_hover_text("Which side of the shell reaches further");
                ui.add(egui::Slider::new(twist, -1.0..=1.0).text("Swirl").fixed_decimals(2));
                ui.add(egui::Slider::new(scallop, 0.0..=0.3).text("Scallop").fixed_decimals(2));
            }
            Kind::Rosette { turn, r, petals, .. } => {
                mm_slider(ui, r, 2.0..=40.0, "Size");
                ui.add(egui::Slider::new(petals, 3..=9).text("Petals"));
                angle(ui, turn, "Turn");
            }
            Kind::Run { outer, count, length, fingers, last, .. } => {
                ui.add(egui::Slider::new(count, 1..=8).text("Fronds"));
                mm_slider(ui, length, 10.0..=150.0, "Length");
                ui.add(egui::Slider::new(fingers, 1..=5).text("Fingers"));
                ui.label("Turnover (second-to-last frond)");
                turnover_choice(ui, t, last);
                if ui.button("Other side of the rim").clicked() { *outer = -*outer; }
            }
            Kind::Frill { across, side, depth, waves, broken, .. } => {
                mm_slider(ui, depth, 2.0..=60.0, "Depth");
                ui.add(egui::Slider::new(waves, 2..=16).text("Crimps"));
                ui.add(egui::Slider::new(broken, 0.0..=1.0).text("Broken edge").fixed_decimals(2)).on_hover_text("Some crests stand shorter");
                if ui.button("Other side of the rim").clicked() { *across = -*across; *side = -*side; }
            }
            Kind::Pocket { lip, .. } => {
                mm_slider(ui, lip, 0.0..=5.0, "Lip");
                ui.label(egui::RichText::new("Drag the points to shape the hollow.").small().color(t.dim));
            }
            Kind::Cabochon { turn, rx, ry, .. } => {
                let (mut w, mut h) = (*rx * 2.0, *ry * 2.0);
                if ui.add(egui::Slider::new(&mut w, 4.0..=120.0).text("Width").suffix(" mm").fixed_decimals(1)).changed() { *rx = w / 2.0; }
                if ui.add(egui::Slider::new(&mut h, 3.0..=120.0).text("Height").suffix(" mm").fixed_decimals(1)).changed() { *ry = h / 2.0; }
                angle(ui, turn, "Angle");
            }
            Kind::Trellis { cell, florets, .. } => {
                mm_slider(ui, cell, 3.0..=25.0, "Cell size");
                ui.checkbox(florets, "Florets at the crossings");
                ui.label(egui::RichText::new("Drag the points to shape the field; Alt-click isn't needed, its outline is smooth through them.").small().color(t.dim));
            }
        }
        if k != e.kind { self.rococo.edit(e.id, k); }
        // ornament that grows out of its rim is one surface with it
        if e.can_grow() && e.rim().is_some() {
            let mut grown = e.grown;
            if ui.checkbox(&mut grown, "Grows from the rim").on_hover_text("Drawn as one surface with its rim, with no outline where they join").changed() {
                let mut d = self.rococo.design.clone();
                if let Some(x) = d.get_mut(e.id) { x.grown = grown; }
                self.rococo.change(d);
            }
        }
        if e.is_rim() {
            let on: Vec<u32> = self.rococo.design.elements.iter().filter(|x| x.rim() == Some(e.id) && x.can_grow()).map(|x| x.id).collect();
            if !on.is_empty() {
                let all = on.iter().all(|id| self.rococo.design.get(*id).is_some_and(|x| x.grown));
                let mut grow = all;
                if ui.checkbox(&mut grow, "Its ornament grows from it").on_hover_text("Fronds, shells, runs and crests on this rim become one surface with it").changed() {
                    let mut d = self.rococo.design.clone();
                    for x in d.elements.iter_mut() { if on.contains(&x.id) { x.grown = grow; } }
                    self.rococo.change(d);
                }
            }
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("Bring to front").clicked() { self.rococo.restack(true); }
            if ui.button("Send to back").clicked() { self.rococo.restack(false); }
            if ui.button("Delete").clicked() { self.rococo.remove_selected(); }
        });
    }

    fn rococo_page(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let mut inches = self.prefs.inches;
        let d = &self.rococo.design;
        let picked = page_sizes::size_editor(ui, t, &mut self.page_sizes, &mut inches, d.width, d.height, 40.0..=1000.0, false);
        if inches != self.prefs.inches { self.set_prefs(Prefs { inches, ..self.prefs }); }
        if let Some((w, h)) = picked {
            // the design stays centred on the resized page
            let mut d = self.rococo.design.clone();
            d.map(1.0, false, 0.0, pt((w - d.width) / 2.0, (h - d.height) / 2.0));
            d.width = w; d.height = h;
            self.rococo.change(d); self.rococo.fitted = false;
        }
        ui.horizontal(|ui| {
            if ui.button("Fit design to page").on_hover_text("Scale and centre the design on the page").clicked() { let mut d = self.rococo.design.clone(); d.fit(); self.rococo.change(d); }
            if ui.button("Fit page to design").on_hover_text("Size the page round the design with a margin").clicked() {
                if let Some(b) = self.rococo.design.bounds() {
                    let m = 10.0; let mut d = self.rococo.design.clone();
                    d.map(1.0, false, 0.0, pt(m - b.l, m - b.t));
                    d.width = ((b.r - b.l + 2.0 * m).ceil()).clamp(40.0, 1000.0); d.height = ((b.b - b.t + 2.0 * m).ceil()).clamp(40.0, 1000.0);
                    self.rococo.change(d); self.rococo.fitted = false;
                }
            }
        });
    }

    pub(crate) fn rococo_canvas(&mut self, ui: &mut egui::Ui) {
        self.rococo.refresh();
        self.rococo.autosave();
        let cc = self.canvas_colors();
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        let st = &mut self.rococo;
        if !st.fitted && rect.width() > 120.0 && rect.height() > 120.0 { st.fit(rect); }
        if let Some(hover) = resp.hover_pos() {
            let (scroll, zoom_delta) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = if zoom_delta != 1.0 { zoom_delta } else { (scroll * 0.0015).exp() };
            if (factor - 1.0).abs() > 1e-4 { let before = st.to_mm(hover); st.zoom = (st.zoom * factor).clamp(0.2, 60.0); let after = st.to_screen(before); st.origin += hover - after; }
            st.cursor_mm = Some(st.to_mm(hover));
        }
        let (pw, ph) = (st.design.width, st.design.height);
        let page = Rect::from_min_max(st.to_screen(pt(0.0, 0.0)), st.to_screen(pt(pw, ph)));
        for (grow, alpha) in [(10.0, 10u8), (5.0, 16), (2.0, 24)] { painter.rect_filled(page.expand(grow).translate(Vec2::new(0.0, grow * 0.4)), grow + 2.0, Color32::from_black_alpha(alpha)); }
        painter.rect_filled(page, 2.0, cc.paper);
        if st.show_grid && 10.0 * st.zoom > 6.0 {
            let mut v = 0.0; let mut k = 0;
            while v <= pw.max(ph) + 1e-9 {
                let w = if k % 5 == 0 { 0.9 } else { 0.5 };
                if v <= pw { let a = st.to_screen(pt(v, 0.0)); painter.line_segment([Pos2::new(a.x, page.top()), Pos2::new(a.x, page.bottom())], Stroke::new(w, cc.grid)); }
                if v <= ph { let b = st.to_screen(pt(0.0, v)); painter.line_segment([Pos2::new(page.left(), b.y), Pos2::new(page.right(), b.y)], Stroke::new(w, cc.grid)); }
                v += 10.0; k += 1;
            }
        }
        // the filled preview, redrawn when the design changes or the zoom moves on
        if st.filled {
            let k = ((st.zoom * ui.ctx().pixels_per_point()) as f64).min(3000.0 / pw.max(ph)).max(0.5);
            let redo = st.texture_stale || st.texture.as_ref().is_none_or(|(_, tk)| (k / tk - 1.0).abs() > 0.2);
            if redo {
                let img = st.raster(k);
                match st.texture.as_mut() { Some((tex, tk)) => { tex.set(img, egui::TextureOptions::LINEAR); *tk = k; } None => st.texture = Some((ui.ctx().load_texture("rococo-filled", img, egui::TextureOptions::LINEAR), k)) }
                st.texture_stale = false;
            }
            if let Some((tex, _)) = &st.texture { painter.image(tex.id(), page, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE); }
        }
        let ink = if st.filled { CARVED_INK } else { cc.ink };
        let sw = (0.3 * st.zoom).clamp(0.8, 2.0);
        if let Some(d) = &st.drawing {
            for run in &d.outlines { if run.len() > 1 { painter.add(Shape::line(run.iter().map(|p| st.to_screen(*p)).collect(), Stroke::new(sw, ink))); } }
            for run in &d.lines { if run.len() > 1 { painter.add(Shape::line(run.iter().map(|p| st.to_screen(*p)).collect(), Stroke::new(sw * 0.6, with_alpha(ink, 210)))); } }
        }
        // hover and selection outlines
        let hover = resp.hover_pos();
        let hovered = if st.drag.is_none() { hover.and_then(|h| st.hit(st.to_mm(h))) } else { None };
        for (id, col, w) in [(hovered.filter(|h| Some(*h) != st.selected), with_alpha(cc.mark, 110), 1.2), (st.selected, cc.mark, 1.8)] {
            let Some(b) = id.and_then(|id| st.built.get(&id)) else { continue };
            for p in &b.parts { if !p.outline.is_empty() { painter.add(Shape::closed_line(p.outline.iter().map(|q| st.to_screen(*q)).collect(), Stroke::new(w, col))); } }
        }
        // the selected rim's control polygon; aim lines; handles
        let handles: Vec<(Handle, Pos2)> = st.handles().into_iter().map(|(h, p)| (h, st.to_screen(p))).collect();
        if let Some(e) = st.selected.and_then(|id| st.design.get(id)) {
            match &e.kind {
                Kind::Rim { nodes, .. } => {
                    // the handle arms, dashed, as on a scroll backbone
                    let last = nodes.len().saturating_sub(1);
                    for (k, n) in nodes.iter().enumerate() {
                        if k > 0 { painter.extend(Shape::dashed_line(&[st.to_screen(n.a), st.to_screen(n.p)], Stroke::new(1.0, cc.guide), 3.0, 3.0)); }
                        if k < last { painter.extend(Shape::dashed_line(&[st.to_screen(n.p), st.to_screen(n.b)], Stroke::new(1.0, cc.guide), 3.0, 3.0)); }
                    }
                }
                Kind::Pocket { ctrl, .. } | Kind::Trellis { ctrl, .. } => { let mut pts: Vec<Pos2> = ctrl.iter().map(|p| st.to_screen(*p)).collect(); if let Some(f) = pts.first().copied() { pts.push(f); } painter.extend(Shape::dashed_line(&pts, Stroke::new(1.0, cc.guide), 5.0, 4.0)); }
                Kind::Frond { at, .. } | Kind::Shell { at, .. } | Kind::Cabochon { at, .. } => if let (Some((p, _)), Some((_, a))) = (Design::place(&st.spines, at), handles.first()) { painter.extend(Shape::dashed_line(&[st.to_screen(p), *a], Stroke::new(1.0, cc.guide), 5.0, 4.0)); },
                _ => {}
            }
        }
        // points solid, handles hollow (as on a scroll backbone); the picked point is ringed
        for (h, p) in &handles {
            let hot = hover.is_some_and(|q| q.distance(*p) < 10.0);
            let grow = if hot { 1.5 } else { 0.0 };
            match h {
                Handle::In(_) | Handle::Out(_) => { painter.circle(*p, 4.5 + grow, cc.paper, Stroke::new(1.5, cc.mark)); }
                Handle::Twist(_) => {
                    // a diamond: where the rim turns over
                    let r = 6.0 + grow;
                    let pts = vec![*p + Vec2::new(0.0, -r), *p + Vec2::new(r, 0.0), *p + Vec2::new(0.0, r), *p + Vec2::new(-r, 0.0)];
                    painter.add(Shape::convex_polygon(pts, if st.handle == Some(*h) { cc.mark } else { cc.paper }, Stroke::new(1.5, cc.mark)));
                }
                Handle::Node(_) => {
                    painter.circle(*p, 5.5 + grow, cc.mark, Stroke::new(1.5, cc.paper));
                    if st.handle == Some(*h) { painter.circle_stroke(*p, 9.0, Stroke::new(1.5, cc.mark)); }
                }
                _ => { painter.circle(*p, 5.5 + grow, cc.paper, Stroke::new(2.0, cc.mark)); }
            }
        }

        // ----- interaction -----
        let pan_button = ui.input(|i| i.pointer.middle_down() || i.pointer.secondary_down() || i.key_down(egui::Key::Space));
        if resp.drag_started() {
            let pos = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos()).unwrap_or_default();
            let mm = st.to_mm(pos);
            st.drag_before = Some(st.design.clone());
            st.drag_spines = st.spines.clone();
            st.drag = if pan_button { Some(RDrag::Pan) }
                else if let (Some(id), Some((h, _))) = (st.selected, handles.iter().find(|(_, p)| p.distance(pos) < 10.0)) { st.handle = Some(*h); Some(RDrag::Handle { id, h: *h }) }
                else if let Some(id) = st.hit(mm) { if st.selected != Some(id) { st.handle = None; } st.selected = Some(id); Some(RDrag::Body { id, start: mm }) }
                else { None };
            st.message.clear();
        }
        if resp.dragged() {
            match st.drag {
                Some(RDrag::Pan) => st.origin += resp.drag_delta(),
                Some(_) => if let Some(pos) = resp.interact_pointer_pos() { let mm = st.to_mm(pos); st.drag_to(mm); },
                None => {}
            }
        }
        if resp.drag_stopped() {
            if let Some(before) = st.drag_before.take() {
                if before != st.design && !matches!(st.drag, Some(RDrag::Pan)) { st.past.push(before); if st.past.len() > 80 { st.past.remove(0); } st.future.clear(); st.dirty = true; st.last_edit = None; }
            }
            st.drag = None;
        }
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let mm = st.to_mm(pos);
                // Alt-click on a rim adds a point there
                if ui.input(|i| i.modifiers.alt) {
                    if let Some(id) = st.hit_rim(mm) { st.insert_point(id, mm); }
                }
                else if let Some((h, _)) = handles.iter().find(|(_, p)| p.distance(pos) < 10.0) { st.handle = Some(*h); }
                else { let id = st.hit(st.to_mm(pos)); if id != st.selected { st.handle = None; } st.selected = id; }
                st.message.clear();
            }
        }
        if handles.iter().any(|(_, p)| hover.is_some_and(|h| h.distance(*p) < 10.0)) { ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair); }
        else if hovered.is_some() { ui.ctx().set_cursor_icon(egui::CursorIcon::Move); }
    }
}

// ---- the shelf layout (ZBrush-style) for the Rococo workspace ----------------

impl App {
    /// Add buttons, the view and Export.
    pub(crate) fn rococo_shelf_context(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::TopBottomPanel::top("rococo-shelf").exact_height(58.0).frame(egui::Frame::none().fill(t.panel).inner_margin(egui::Margin::symmetric(10.0, 0.0)).stroke(Stroke::new(1.0, t.border))).show(ctx, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if big_tab(ui, t, "Export", false).on_hover_text("Save the pattern at actual size as SVG").clicked() { self.rococo.export(); }
                ui.add_space(8.0); divider(ui, t); ui.add_space(8.0);
                ui.allocate_ui_with_layout(Vec2::new(124.0, 50.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.add_space(3.0);
                    ui.label(egui::RichText::new("VIEW").size(10.5).color(t.dim));
                    let mut filled = self.rococo.filled;
                    if seg(ui, t, &[(false, "Lines"), (true, "Filled")], &mut filled, 58.0, 22.0) { self.rococo.filled = filled; }
                });
                ui.add_space(4.0); divider(ui, t); ui.add_space(8.0);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    sh::tight_controls(ui, |ui| {
                        caption(ui, t, "Add");
                        let on_rim = self.rococo.current_rim().is_some();
                        for (what, name, w, needs_rim, tip) in [("rim", "Rim", 52.0, false, "A new moulded rim in the middle of the page"), ("frond", "Frond", 62.0, true, "A frond on the selected rim"), ("shell", "Shell", 58.0, true, "A fluted shell on the selected rim"), ("rosette", "Rosette", 72.0, true, "A rosette in the selected rim's channel"), ("run", "Run", 50.0, true, "A run of overlapping fronds along the selected rim"), ("crest", "Crest", 58.0, true, "A crimped crest along the selected rim"), ("cabochon", "Cabochon", 82.0, true, "A convex oval boss on the selected rim"), ("pocket", "Pocket", 66.0, false, "A pierced hollow, behind everything"), ("trellis", "Trellis", 66.0, false, "A lattice field with florets, behind everything; drag its points to shape it")] {
                            let resp = bevel_button(ui, t, name, false, Vec2::new(w, 38.0));
                            let resp = if needs_rim && !on_rim { resp.on_hover_text(format!("{tip}\nSelect a rim (or something on it) first")) } else { resp.on_hover_text(tip) };
                            if resp.clicked() { self.rococo.add(what); }
                        }
                    });
                });
            });
        });
    }

    /// The library shelf: the starting layouts, drawn from their rims.
    pub(crate) fn rococo_shelf_tools(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::left("rococo-shelf-tools").exact_width(84.0).resizable(false).frame(egui::Frame::none().fill(t.panel).inner_margin(egui::Margin::symmetric(8.0, 10.0)).stroke(Stroke::new(1.0, t.border))).show(ctx, |ui| {
            egui::ScrollArea::vertical().scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                shelf_caption(ui, t, "LIBRARY");
                let thumbs = self.rococo.thumbs().clone();
                for ((id, name, detail), (lines, shells)) in STRUCTURES.iter().zip(thumbs) {
                    let on = self.rococo.structure == *id;
                    let resp = tile_with(ui, t, name, on, |p, r, c| {
                        let r = r.expand(4.0);
                        let at = |q: &Point| Pos2::new(r.left() + q.x as f32 * r.width(), r.top() + q.y as f32 * r.height());
                        for l in &lines { p.add(Shape::line(l.iter().map(at).collect(), Stroke::new(1.5, c))); }
                        for s in &shells { p.circle_filled(at(s), 3.5, c); }
                    }).on_hover_text(*detail);
                    if resp.clicked() { self.rococo.use_structure(id); }
                }
            });
        });
    }

    /// Folding palettes: the selection, the page and the canvas.
    pub(crate) fn rococo_shelf_tray(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::right("rococo-shelf-tray").default_width(340.0).min_width(300.0).frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin { left: 8.0, right: 6.0, top: 8.0, bottom: 8.0 })).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(ui.available_width() - 6.0);
                ui.spacing_mut().item_spacing.y = 6.0;
                let title = self.rococo.selected.and_then(|id| self.rococo.design.get(id)).map_or("Selection".to_string(), |e| format!("Selected {}", e.label().to_lowercase()));
                self.palette(ui, sh::ROCOCO_SELECTION, &title, |s, ui| s.rococo_selected(ui));
                self.palette(ui, sh::ROCOCO_ADD, "Add", |s, ui| s.rococo_add(ui));
                self.palette(ui, sh::ROCOCO_PAGE, "Page", |s, ui| s.rococo_page(ui));
                self.palette(ui, sh::CANVAS, "Canvas", |s, ui| { ui.checkbox(&mut s.rococo.show_grid, "Millimetre grid"); s.theme_tab(ui) });
            });
        });
    }

    pub(crate) fn rococo_shelf_status(&mut self, ctx: &egui::Context) {
        let t = self.t();
        let hint = self.rococo_hint();
        egui::TopBottomPanel::bottom("rococo-shelf-status").exact_height(28.0).frame(egui::Frame::none().fill(t.bg).inner_margin(egui::Margin::symmetric(14.0, 0.0))).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if sh::small_button(ui, t, "Fit").on_hover_text("Fit the page in the window").clicked() { self.rococo.fit_page(); }
                    ui.label(egui::RichText::new(format!("Zoom {:.0}%", self.rococo.zoom / 3.78 * 100.0)).size(12.5).color(t.dim));
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let st = &self.rococo;
                        let s = |x: String| egui::RichText::new(x).size(12.5).color(t.dim);
                        ui.label(s(format!("{} × {} mm · {} elements", st.design.width, st.design.height, st.design.elements.len()))); ui.add_space(12.0);
                        if let Some(m) = st.cursor_mm { ui.label(s(format!("x {:.1}  y {:.1} mm", m.x, m.y))); ui.add_space(12.0); }
                        ui.add(egui::Label::new(s(hint)).truncate());
                    });
                });
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(id: &str) -> RococoState {
        let mut st = RococoState::new();
        st.design = Design::from_structure(id, 240.0, 250.0).unwrap();
        st.refresh();
        st
    }
    /// Drag `drag` from `from` to `to`, as the canvas does.
    fn drag(st: &mut RococoState, drag: RDrag, to: Point) {
        st.drag_before = Some(st.design.clone()); st.drag_spines = st.spines.clone(); st.drag = Some(drag);
        st.drag_to(to);
        st.drag = None; st.refresh();
    }

    #[test]
    fn ornament_slides_along_its_rim() {
        let mut st = state("corner");
        let e = st.design.elements.iter().find(|e| matches!(e.kind, Kind::Frond { at: Anchor::Rim { .. }, .. })).unwrap().clone();
        let Kind::Frond { at: Anchor::Rim { rim, u, along, across }, .. } = e.kind else { unreachable!() };
        let s = st.spines[&rim].clone();
        let (p, h) = s.frame(u);
        // pull it along the rim and off it: it stays on the rim, the same distance across
        let du = if u > 0.5 { -0.15 } else { 0.15 }; // toward the middle, clear of the volutes
        let (a, b) = (s.frame(u).0, s.frame(u + du).0);
        let to = pt(b.x - h.sin() * 5.0, b.y + h.cos() * 5.0);
        drag(&mut st, RDrag::Body { id: e.id, start: a }, to);
        let Kind::Frond { at: Anchor::Rim { rim: r2, u: u2, along: al2, across: ac2 }, .. } = st.design.get(e.id).unwrap().kind else { panic!("left its rim") };
        assert_eq!(r2, rim);
        assert!((al2 - along).abs() < 1e-9 && (ac2 - across).abs() < 1e-9);
        assert!((u2 - (u + du)).abs() < 0.03, "u {u} -> {u2}");
        let _ = p;
    }

    #[test]
    fn turn_overs_drag_along_the_rim_and_cabochons_aim() {
        let mut st = state("corner");
        let rim = st.design.elements.iter().find(|e| e.is_rim()).unwrap().id;
        if let Some(Kind::Rim { twists, .. }) = st.design.get_mut(rim).map(|e| &mut e.kind) { twists.push(0.2); }
        st.refresh();
        let to = st.spines[&rim].frame(0.6).0;
        drag(&mut st, RDrag::Handle { id: rim, h: Handle::Twist(0) }, to);
        let Kind::Rim { twists, .. } = &st.design.get(rim).unwrap().kind else { unreachable!() };
        assert!((twists[0] - 0.6).abs() < 0.02, "turn-over at {}", twists[0]);
        // a cabochon's handle turns it and sizes it, keeping its proportions
        let mut d = st.design.clone(); let c = d.add_cabochon(rim, 0.5).unwrap(); st.design = d; st.refresh();
        let Some(Kind::Cabochon { at, rx, ry, .. }) = st.design.get(c).map(|e| e.kind.clone()) else { unreachable!() };
        let (p, _) = Design::place(&st.spines, &at).unwrap();
        drag(&mut st, RDrag::Handle { id: c, h: Handle::Aim }, pt(p.x, p.y - rx * 2.0));
        let Some(Kind::Cabochon { rx: rx2, ry: ry2, .. }) = st.design.get(c).map(|e| e.kind.clone()) else { unreachable!() };
        assert!((rx2 - rx * 2.0).abs() < 1e-6 && (ry2 / rx2 - ry / rx).abs() < 1e-9);
    }

    #[test]
    fn rim_points_and_handles_drag_and_alt_click_adds_a_point() {
        let mut st = state("corner");
        let rim = st.design.elements.iter().find(|e| e.is_rim()).unwrap().id;
        let Kind::Rim { nodes, .. } = st.design.get(rim).unwrap().kind.clone() else { unreachable!() };
        // a point carries its handles
        let n1 = nodes[1];
        drag(&mut st, RDrag::Handle { id: rim, h: Handle::Node(1) }, pt(n1.p.x + 10.0, n1.p.y + 5.0));
        let Kind::Rim { nodes: m, .. } = st.design.get(rim).unwrap().kind.clone() else { unreachable!() };
        assert!((m[1].a.x - n1.a.x - 10.0).abs() < 1e-9 && (m[1].b.y - n1.b.y - 5.0).abs() < 1e-9);
        // an out-handle swings the in-handle round to stay in line
        let to = pt(m[1].p.x + 20.0, m[1].p.y - 20.0);
        drag(&mut st, RDrag::Handle { id: rim, h: Handle::Out(1) }, to);
        let Kind::Rim { nodes: m2, .. } = st.design.get(rim).unwrap().kind.clone() else { unreachable!() };
        let (q, a) = (m2[1].p, m2[1].a);
        let cross = (to.x - q.x) * (a.y - q.y) - (to.y - q.y) * (a.x - q.x);
        assert!(cross.abs() < 1e-6 && (to.x - q.x) * (a.x - q.x) < 0.0);
        // Alt-click on the rim body adds a point there
        let s = st.spines[&rim].clone();
        let on = s.frame(0.3).0;
        assert_eq!(st.hit_rim(on), Some(rim));
        st.insert_point(rim, on);
        let Kind::Rim { nodes: m3, .. } = st.design.get(rim).unwrap().kind.clone() else { unreachable!() };
        assert_eq!(m3.len(), m2.len() + 1);
        assert!(matches!(st.handle, Some(Handle::Node(_))));
    }
}
