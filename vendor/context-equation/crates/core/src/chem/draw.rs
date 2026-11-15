//! What a structure or scheme is drawn with, in font units (1000 per em), y pointing up: strokes,
//! polygons, circles, cubic curves and boxes of text. At the end it all becomes one `MathBox` made
//! of the items every equation uses (lines, ellipses, glyphs): a curve is sampled into short strokes,
//! a filled wedge is a fan of strokes, a dash pattern is its dashes.

use crate::layout::{Item as MathItem, MathBox};

pub type P = (f32, f32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Rect {
    pub fn around(p: P, r: f32) -> Rect {
        Rect { x0: p.0 - r, y0: p.1 - r, x1: p.0 + r, y1: p.1 + r }
    }

    pub fn union(self, o: Rect) -> Rect {
        Rect { x0: self.x0.min(o.x0), y0: self.y0.min(o.y0), x1: self.x1.max(o.x1), y1: self.y1.max(o.y1) }
    }

    pub fn center(&self) -> P {
        ((self.x0 + self.x1) / 2.0, (self.y0 + self.y1) / 2.0)
    }

    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> f32 {
        self.y1 - self.y0
    }

    pub fn grow(self, m: f32) -> Rect {
        Rect { x0: self.x0 - m, y0: self.y0 - m, x1: self.x1 + m, y1: self.y1 + m }
    }

    pub fn shifted(self, d: P) -> Rect {
        Rect { x0: self.x0 + d.0, y0: self.y0 + d.1, x1: self.x1 + d.0, y1: self.y1 + d.1 }
    }

    /// Distance from the center to the border along the unit direction `d`.
    pub fn reach(&self, d: P) -> f32 {
        let (hw, hh) = (self.width() / 2.0, self.height() / 2.0);
        let tx = if d.0.abs() > 1e-6 { hw / d.0.abs() } else { f32::INFINITY };
        let ty = if d.1.abs() > 1e-6 { hh / d.1.abs() } else { f32::INFINITY };
        tx.min(ty)
    }

    /// Where the ray from `p` (inside) along the unit direction `d` leaves the rectangle.
    pub fn exit(&self, p: P, d: P) -> P {
        let mut t = f32::INFINITY;
        if d.0 > 1e-6 {
            t = t.min((self.x1 - p.0) / d.0);
        } else if d.0 < -1e-6 {
            t = t.min((self.x0 - p.0) / d.0);
        }
        if d.1 > 1e-6 {
            t = t.min((self.y1 - p.1) / d.1);
        } else if d.1 < -1e-6 {
            t = t.min((self.y0 - p.1) / d.1);
        }
        if !t.is_finite() {
            t = 0.0;
        }
        add(p, mul(d, t.max(0.0)))
    }
}

pub fn add(a: P, b: P) -> P {
    (a.0 + b.0, a.1 + b.1)
}

pub fn sub(a: P, b: P) -> P {
    (a.0 - b.0, a.1 - b.1)
}

pub fn mul(a: P, f: f32) -> P {
    (a.0 * f, a.1 * f)
}

pub fn length(a: P) -> f32 {
    a.0.hypot(a.1)
}

pub fn unit(a: P) -> P {
    let l = length(a);
    if l < 1e-6 {
        (1.0, 0.0)
    } else {
        (a.0 / l, a.1 / l)
    }
}

/// The left normal of a direction.
pub fn left(d: P) -> P {
    (-d.1, d.0)
}

pub fn direction(degrees: f32) -> P {
    let (s, c) = degrees.to_radians().sin_cos();
    (c, s)
}

pub fn rotate(d: P, degrees: f32) -> P {
    let (s, c) = degrees.to_radians().sin_cos();
    (d.0 * c - d.1 * s, d.0 * s + d.1 * c)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Line {
        a: P,
        b: P,
        width: f32,
        dash: Option<(f32, f32)>,
    },
    /// Closed polygon, filled or stroked.
    Polygon {
        points: Vec<P>,
        fill: bool,
        width: f32,
    },
    Circle {
        center: P,
        radius: f32,
        width: f32,
        fill: bool,
    },
    /// Cubic Bézier, stroked.
    Curve {
        a: P,
        c1: P,
        c2: P,
        b: P,
        width: f32,
    },
    /// Text typeset by contextEquation, its baseline origin at `at`.
    Math {
        math_box: MathBox,
        at: P,
    },
}

/// Shapes and the box that holds them.
#[derive(Debug, Clone, Default)]
pub struct Drawing {
    pub shapes: Vec<Shape>,
    pub bounds: Option<Rect>,
}

impl Drawing {
    pub fn grow(&mut self, r: Rect) {
        self.bounds = Some(match self.bounds {
            None => r,
            Some(b) => b.union(r),
        });
    }

    pub fn bounds(&self) -> Rect {
        self.bounds.unwrap_or(Rect { x0: 0.0, y0: 0.0, x1: 0.0, y1: 0.0 })
    }

    pub fn line(&mut self, a: P, b: P, width: f32) {
        self.grow(Rect::around(a, width).union(Rect::around(b, width)));
        self.shapes.push(Shape::Line { a, b, width, dash: None });
    }

    pub fn dashed(&mut self, a: P, b: P, width: f32, dash: (f32, f32)) {
        self.grow(Rect::around(a, width).union(Rect::around(b, width)));
        self.shapes.push(Shape::Line { a, b, width, dash: Some(dash) });
    }

    pub fn polygon(&mut self, points: Vec<P>, fill: bool, width: f32) {
        for p in &points {
            self.grow(Rect::around(*p, width));
        }
        self.shapes.push(Shape::Polygon { points, fill, width });
    }

    pub fn circle(&mut self, center: P, radius: f32, width: f32, fill: bool) {
        self.grow(Rect::around(center, radius + width));
        self.shapes.push(Shape::Circle { center, radius, width, fill });
    }

    /// An arc of the circle around `center`, from `from` to `to` degrees counterclockwise, as short
    /// strokes; with `dash` (on, off), only the strokes that fall on a dash.
    pub fn arc(&mut self, center: P, radius: f32, from: f32, to: f32, width: f32, dash: Option<(f32, f32)>) {
        let sweep = to - from;
        let steps = ((sweep.abs() / 3.0).ceil() as usize).max(2);
        let point = |t: f32| add(center, mul(direction(from + sweep * t), radius));
        let step_length = radius * sweep.abs().to_radians() / steps as f32;
        let mut travelled = 0.0;
        let mut prev = point(0.0);
        for k in 1..=steps {
            let p = point(k as f32 / steps as f32);
            let on = dash.is_none_or(|(on, off)| (travelled + step_length / 2.0) % (on + off) < on);
            if on {
                self.line(prev, p, width);
            }
            travelled += step_length;
            prev = p;
        }
    }

    pub fn curve(&mut self, a: P, c1: P, c2: P, b: P, width: f32) {
        // the curve stays inside the hull of its four points; sampling gives a tighter box
        for i in 0..=16 {
            self.grow(Rect::around(cubic(a, c1, c2, b, i as f32 / 16.0), width));
        }
        self.shapes.push(Shape::Curve { a, c1, c2, b, width });
    }

    pub fn math(&mut self, math_box: MathBox, at: P) {
        if math_box.width > 0.0 || math_box.total() > 0.0 {
            self.grow(Rect { x0: at.0, y0: at.1 - math_box.depth, x1: at.0 + math_box.width, y1: at.1 + math_box.height });
        }
        self.shapes.push(Shape::Math { math_box, at });
    }

    /// Adds another drawing moved by `d`.
    pub fn place(&mut self, other: &Drawing, d: P) {
        for s in &other.shapes {
            self.shapes.push(shift(s, d));
        }
        if let Some(b) = other.bounds {
            self.grow(b.shifted(d));
        }
    }

    /// The same drawing `f` times as large, from the origin.
    pub fn scaled(&self, f: f32) -> Drawing {
        let s = |p: P| mul(p, f);
        let shapes = self
            .shapes
            .iter()
            .map(|shape| match shape {
                Shape::Line { a, b, width, dash } => Shape::Line { a: s(*a), b: s(*b), width: width * f, dash: dash.map(|(on, off)| (on * f, off * f)) },
                Shape::Polygon { points, fill, width } => Shape::Polygon { points: points.iter().map(|p| s(*p)).collect(), fill: *fill, width: width * f },
                Shape::Circle { center, radius, width, fill } => Shape::Circle { center: s(*center), radius: radius * f, width: width * f, fill: *fill },
                Shape::Curve { a, c1, c2, b, width } => Shape::Curve { a: s(*a), c1: s(*c1), c2: s(*c2), b: s(*b), width: width * f },
                Shape::Math { math_box, at } => {
                    let mut m = math_box.clone();
                    m.scale_box(f);
                    Shape::Math { math_box: m, at: s(*at) }
                }
            })
            .collect();
        Drawing { shapes, bounds: self.bounds.map(|b| Rect { x0: b.x0 * f, y0: b.y0 * f, x1: b.x1 * f, y1: b.y1 * f }) }
    }
}

pub fn cubic(a: P, c1: P, c2: P, b: P, t: f32) -> P {
    let u = 1.0 - t;
    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
    (w[0] * a.0 + w[1] * c1.0 + w[2] * c2.0 + w[3] * b.0, w[0] * a.1 + w[1] * c1.1 + w[2] * c2.1 + w[3] * b.1)
}

fn shift(s: &Shape, d: P) -> Shape {
    match s {
        Shape::Line { a, b, width, dash } => Shape::Line { a: add(*a, d), b: add(*b, d), width: *width, dash: *dash },
        Shape::Polygon { points, fill, width } => Shape::Polygon { points: points.iter().map(|p| add(*p, d)).collect(), fill: *fill, width: *width },
        Shape::Circle { center, radius, width, fill } => Shape::Circle { center: add(*center, d), radius: *radius, width: *width, fill: *fill },
        Shape::Curve { a, c1, c2, b, width } => Shape::Curve { a: add(*a, d), c1: add(*c1, d), c2: add(*c2, d), b: add(*b, d), width: *width },
        Shape::Math { math_box, at } => Shape::Math { math_box: math_box.clone(), at: add(*at, d) },
    }
}

/// Strokes per curve: enough for a smooth line at any display size.
const CURVE_STEPS: usize = 24;

fn line(items: &mut Vec<MathItem>, a: P, b: P, width: f32) {
    items.push(MathItem::Line { x1: a.0, y1: a.1, x2: b.0, y2: b.1, thickness: width, color: None });
}

impl Drawing {
    /// The drawing as a box: its left edge at x = 0, y unchanged (the caller centers it).
    pub fn to_box(&self) -> MathBox {
        let b = self.bounds();
        let dx = -b.x0;
        let mut items: Vec<MathItem> = Vec::new();
        let at = |p: P| (p.0 + dx, p.1);
        for shape in &self.shapes {
            match shape {
                Shape::Line { a, b, width, dash: None } => line(&mut items, at(*a), at(*b), *width),
                Shape::Line { a, b, width, dash: Some((on, off)) } => {
                    let total = length(sub(*b, *a));
                    let u = unit(sub(*b, *a));
                    let mut s = 0.0;
                    while s < total {
                        let e = (s + on).min(total);
                        line(&mut items, at(add(*a, mul(u, s))), at(add(*a, mul(u, e))), *width);
                        s += on + off;
                    }
                }
                Shape::Polygon { points, fill, width } => {
                    let n = points.len();
                    for i in 0..n {
                        line(&mut items, at(points[i]), at(points[(i + 1) % n]), *width);
                    }
                    // a filled triangle (a wedge) is a fan of strokes from its first corner to the far side
                    if *fill && n == 3 {
                        let side = length(sub(points[2], points[1]));
                        let count = ((side / (width * 0.6)).ceil() as usize).max(2);
                        for k in 0..=count {
                            let t = k as f32 / count as f32;
                            let far = add(points[1], mul(sub(points[2], points[1]), t));
                            line(&mut items, at(points[0]), at(far), *width);
                        }
                    }
                }
                Shape::Circle { center, radius, width, fill: false } => {
                    let c = at(*center);
                    items.push(MathItem::Ellipse { cx: c.0, cy: c.1, rx: *radius, ry: *radius, thickness: *width, color: None });
                }
                Shape::Circle { center, radius, fill: true, .. } => {
                    // a dot: a ring as thick as its radius leaves no hole
                    let c = at(*center);
                    items.push(MathItem::Ellipse { cx: c.0, cy: c.1, rx: radius / 2.0, ry: radius / 2.0, thickness: *radius, color: None });
                }
                Shape::Curve { a, c1, c2, b, width } => {
                    let mut prev = *a;
                    for k in 1..=CURVE_STEPS {
                        let p = cubic(*a, *c1, *c2, *b, k as f32 / CURVE_STEPS as f32);
                        line(&mut items, at(prev), at(p), *width);
                        prev = p;
                    }
                }
                Shape::Math { math_box, at: origin } => {
                    let mut m = MathBox::space(0.0);
                    m.place(math_box, origin.0 + dx, origin.1);
                    items.extend(m.items);
                }
            }
        }
        MathBox { width: b.width(), height: b.y1.max(0.0), depth: (-b.y0).max(0.0), items, ..MathBox::default() }
    }
}
