//! Carved boxes built from chip panels: a lid, front, back, two sides and an optional
//! bottom, each a chip pattern sized from the box's outer dimensions. The back follows
//! the front and the right side follows the left until one is edited on its own.
use crate::chip::{chip_drawing, ChipSettings, Faceted};
use crate::facets::{BorderStyle, Centre, Repeat};

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Face { Lid, Front, Back, Left, Right, Bottom }

impl Face {
    pub const ALL: [Face; 6] = [Face::Lid, Face::Front, Face::Back, Face::Left, Face::Right, Face::Bottom];
    pub fn key(self) -> &'static str { match self { Face::Lid => "lid", Face::Front => "front", Face::Back => "back", Face::Left => "left", Face::Right => "right", Face::Bottom => "bottom" } }
    pub fn from_key(k: &str) -> Option<Face> { Face::ALL.into_iter().find(|f| f.key() == k) }
    pub fn label(self) -> &'static str { match self { Face::Lid => "Lid", Face::Front => "Front", Face::Back => "Back", Face::Left => "Left side", Face::Right => "Right side", Face::Bottom => "Bottom" } }
    /// The face this one follows until edited on its own.
    pub fn partner(self) -> Option<Face> { match self { Face::Back => Some(Face::Front), Face::Right => Some(Face::Left), _ => None } }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoxDesign {
    /// Outer dimensions in mm: length (left to right), width (front to back), height.
    pub length: f64, pub width: f64, pub height: f64,
    pub lid: ChipSettings, pub front: ChipSettings, pub left: ChipSettings,
    /// None: the same pattern as the front / the left side.
    pub back: Option<ChipSettings>, pub right: Option<ChipSettings>,
    /// None: the bottom isn't carved.
    pub bottom: Option<ChipSettings>,
}

pub const DIMENSIONS: std::ops::RangeInclusive<f64> = 40.0..=600.0;

/// A side panel matching `like`: its border style, with a field of stars and diamonds inside.
pub fn matching_panel(like: &ChipSettings, w: f64, h: f64) -> ChipSettings {
    let border = like.faceted.and_then(|f| f.border).or(Some(BorderStyle::Zigzag));
    let mut f = Faceted::new(Centre::Star, 8, border); f.field = Some(Repeat::StarsAndDiamonds);
    ChipSettings { faceted: Some(f), seed: like.seed, grid: like.grid, ..ChipSettings::default() }.resized(w, h)
}

/// A lid matching `like`: its border style round an eight-point star.
fn matching_lid(like: &ChipSettings, w: f64, h: f64) -> ChipSettings {
    let border = like.faceted.and_then(|f| f.border).or(Some(BorderStyle::Zigzag));
    ChipSettings { faceted: Some(Faceted::new(Centre::Star, 8, border)), seed: like.seed, grid: like.grid, ..ChipSettings::default() }.resized(w, h)
}

impl BoxDesign {
    /// Width × height of a face's panel, in mm.
    pub fn size_of(&self, face: Face) -> (f64, f64) {
        match face { Face::Lid | Face::Bottom => (self.length, self.width), Face::Front | Face::Back => (self.length, self.height), Face::Left | Face::Right => (self.width, self.height) }
    }

    /// A box grown from one panel. As the lid, the panel gives length × width and
    /// `third` is the height; as the front, it gives length × height and `third` the width.
    /// The other faces start matched to it.
    pub fn from_panel(panel: &ChipSettings, face: Face, third: f64) -> BoxDesign {
        let (pw, ph) = (panel.width(), panel.page_height());
        let third = third.clamp(*DIMENSIONS.start(), *DIMENSIONS.end());
        let (length, width, height) = if face == Face::Front { (pw, third, ph) } else { (pw, ph, third) };
        let lid = if face == Face::Front { matching_lid(panel, length, width) } else { panel.clone() };
        let front = if face == Face::Front { panel.clone() } else { matching_panel(panel, length, height) };
        let left = matching_panel(panel, width, height);
        BoxDesign { length, width, height, lid, front, left, back: None, right: None, bottom: None }
    }

    /// A face's pattern, following its partner while linked; None for an uncarved bottom.
    pub fn panel(&self, face: Face) -> Option<&ChipSettings> {
        match face {
            Face::Lid => Some(&self.lid), Face::Front => Some(&self.front), Face::Left => Some(&self.left),
            Face::Back => Some(self.back.as_ref().unwrap_or(&self.front)),
            Face::Right => Some(self.right.as_ref().unwrap_or(&self.left)),
            Face::Bottom => self.bottom.as_ref(),
        }
    }
    /// True while the back follows the front, or the right side the left.
    pub fn linked(&self, face: Face) -> bool { match face { Face::Back => self.back.is_none(), Face::Right => self.right.is_none(), _ => false } }

    /// Store a face's pattern. Editing a linked back or right side gives it its own pattern.
    pub fn set_panel(&mut self, face: Face, s: ChipSettings) {
        match face {
            Face::Lid => self.lid = s, Face::Front => self.front = s, Face::Left => self.left = s,
            Face::Back => self.back = Some(s), Face::Right => self.right = Some(s), Face::Bottom => self.bottom = Some(s),
        }
    }
    /// Make the back follow the front again (or the right side the left).
    pub fn relink(&mut self, face: Face) { match face { Face::Back => self.back = None, Face::Right => self.right = None, _ => {} } }
    /// Carve the bottom (it starts matched to the lid's border) or leave it plain.
    pub fn set_bottom(&mut self, on: bool) {
        self.bottom = if on { Some(self.bottom.clone().unwrap_or_else(|| matching_panel(&self.lid, self.length, self.width))) } else { None };
    }
    /// The faces that are carved, in order.
    pub fn faces(&self) -> Vec<Face> { Face::ALL.into_iter().filter(|f| self.panel(*f).is_some()).collect() }

    /// The box at new outer dimensions: every panel is regenerated at its new size.
    pub fn resized(&self, length: f64, width: f64, height: f64) -> BoxDesign {
        let mut b = BoxDesign { length, width, height, ..self.clone() };
        for face in Face::ALL {
            let (w, h) = b.size_of(face);
            match face {
                Face::Lid => b.lid = self.lid.resized(w, h), Face::Front => b.front = self.front.resized(w, h), Face::Left => b.left = self.left.resized(w, h),
                Face::Back => b.back = self.back.as_ref().map(|s| s.resized(w, h)),
                Face::Right => b.right = self.right.as_ref().map(|s| s.resized(w, h)),
                Face::Bottom => b.bottom = self.bottom.as_ref().map(|s| s.resized(w, h)),
            }
        }
        b
    }

    /// The assembled box in isometric: for each visible face, its panel and an affine map
    /// [a, b, c, d, e, f] from panel mm (x, y) to view mm (a·x + c·y + e, b·x + d·y + f),
    /// and how much to darken it (faces turned from the light). Seen from the front-right,
    /// or from the back-left. Also returns the view's width and height.
    pub fn iso_view(&self, from_back: bool) -> (Vec<(Face, [f64; 6], f64)>, f64, f64) {
        let (l, w, h) = (self.length, self.width, self.height);
        let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
        // 3D (X along the length, Y toward the front, Z up) to the view, shifted to start at 0
        let proj = |x: f64, y: f64, z: f64| ((x - y) * c + w * c, (x + y) * s - z + h);
        let vec = |x: f64, y: f64, z: f64| ((x - y) * c, (x + y) * s - z);
        let map = |o: (f64, f64, f64), ux: (f64, f64, f64), uy: (f64, f64, f64)| {
            let (e, f) = proj(o.0, o.1, o.2); let (a, b) = vec(ux.0, ux.1, ux.2); let (cc, d) = vec(uy.0, uy.1, uy.2);
            [a, b, cc, d, e, f]
        };
        // from the back-left the box is turned half round: the back takes the front's
        // place, the left side the right's, and the lid turns with it
        let (top, side_a, side_b) = if from_back { (Face::Lid, Face::Back, Face::Left) } else { (Face::Lid, Face::Front, Face::Right) };
        let lid = if from_back { map((l, w, h), (-1.0, 0.0, 0.0), (0.0, -1.0, 0.0)) } else { map((0.0, 0.0, h), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)) };
        let faces = vec![
            (top, lid, 0.0),
            (side_a, map((0.0, w, h), (1.0, 0.0, 0.0), (0.0, 0.0, -1.0)), 0.12),
            (side_b, map((l, w, h), (0.0, -1.0, 0.0), (0.0, 0.0, -1.0)), 0.28),
        ];
        (faces, (l + w) * c, (l + w) * s + h)
    }

    /// Every carved panel on one sheet at real size, labelled: lid (and bottom), front and
    /// back, then the sides.
    pub fn sheet_svg(&self) -> String {
        let gap = 12.0; let mut y = gap; let mut body = String::new(); let mut width: f64 = 0.0;
        let rows: [&[Face]; 3] = [&[Face::Lid, Face::Bottom], &[Face::Front, Face::Back], &[Face::Left, Face::Right]];
        for row in rows {
            let mut x = gap; let mut tallest: f64 = 0.0;
            for &face in row {
                let Some(p) = self.panel(face) else { continue };
                let (w, h) = self.size_of(face);
                body += &format!("<g transform=\"translate({x} {y})\"><text x=\"0\" y=\"-3\" font-family=\"Segoe UI, sans-serif\" font-size=\"4\">{}  {} × {} mm</text><rect width=\"{w}\" height=\"{h}\" fill=\"none\" stroke=\"#999\" stroke-width=\"0.2\" stroke-dasharray=\"1 1\"/>{}</g>", face.label(), w, h, chip_drawing(p));
                x += w + gap; tallest = tallest.max(h);
            }
            width = width.max(x); y += tallest + gap;
        }
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}mm\" height=\"{y}mm\" viewBox=\"0 0 {width} {y}\"><title>Chip carving box {} × {} × {} mm</title>{body}</svg>", self.length, self.width, self.height)
    }
}
