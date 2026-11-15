//! The pen (`context_equation_core::pen`) over every kind of item of the layout: turned boxes, lines, circles,
//! groups, rules standing up, rules with no length, and an equation with nothing to draw.

use context_equation_core::output::pen::Pen;
use context_equation_core::Options;

fn pen(tex: &str) -> String {
    context_equation_core::pen(tex, &Options::default(), &Pen::default()).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

/// The `d` of each stroke the pen goes over.
fn strokes(svg: &str) -> Vec<&str> {
    svg.split(r#"<path d=""#).skip(1).map(|r| r.split('"').next().unwrap()).collect()
}

#[test]
fn a_turned_box_is_drawn_inside_its_matrix() {
    let svg = pen(r"\rotatebox{90}{x}");
    // the element that fills in is wrapped in the turn; the strokes are already turned
    assert!(svg.contains(r#"<g opacity="0"><g transform="matrix("#), "{svg}");
    assert!(svg.contains("<use href=") && !strokes(&svg).is_empty());
}

#[test]
fn a_cancel_line_is_one_stroke_then_a_line() {
    let svg = pen(r"\cancel{x}");
    assert!(svg.contains(r#"<line x1=""#) && svg.contains(r#"stroke="currentColor""#), "{svg}");
    // the glyph's contours and the line
    assert!(strokes(&svg).len() >= 2);
}

#[test]
fn a_circle_is_drawn_around_from_the_top() {
    let svg = pen(r"\textcircled{x}");
    assert!(svg.contains("<ellipse ") && svg.contains(r#"fill="none""#), "{svg}");
    // the ring: 65 points, the last back where it started
    let ring = strokes(&svg).into_iter().find(|d| d.matches(" L").count() == 64).expect("a ring of 64 segments");
    let points: Vec<&str> = ring.trim_start_matches('M').split(" L").collect();
    assert_eq!(points.first(), points.last());
}

#[test]
fn the_attributes_of_a_group_are_left_to_the_plain_svg() {
    let svg = pen(r"\class{foo}{x}");
    assert!(!svg.contains("foo"), "{svg}");
    assert_eq!(svg.matches("<use href=").count(), 1);
    // the plain SVG keeps them
    assert!(context_equation_core::convert(r"\class{foo}{x}", &Options::default()).unwrap().svg.contains(r#"class="foo""#));
}

#[test]
fn a_colored_glyph_fills_in_its_color() {
    let svg = pen(r"\textcolor{red}{x} y");
    assert_eq!(svg.matches(r#" fill="red""#).count(), 1, "{svg}");
    assert_eq!(svg.matches("<use href=").count(), 2);
}

#[test]
fn a_rule_standing_up_is_drawn_from_the_top_down() {
    let svg = pen(r"\rule{1pt}{2em}");
    let d = strokes(&svg);
    assert_eq!(d.len(), 1, "{svg}");
    let points: Vec<Vec<f32>> = d[0].trim_start_matches('M').split(" L").map(|p| p.split(' ').map(|v| v.parse().unwrap()).collect()).collect();
    assert_eq!(points.len(), 2);
    // one x, from the top to the bottom
    assert_eq!(points[0][0], points[1][0]);
    assert!(points[0][1] > points[1][1], "{points:?}");
}

#[test]
fn a_rule_with_no_length_is_placed_without_a_stroke() {
    let svg = pen(r"\rule{0pt}{0pt} x");
    // nothing to go over: the rule is there from the start, not faded in
    assert!(svg.contains(r#"<rect x="0" y="0" width="0" height="0"/>"#), "{svg}");
    assert!(!svg.contains(r#"<g opacity="0"><rect"#));
    assert!(svg.contains(r#"<g opacity="0"><use"#));
}

#[test]
fn an_empty_equation_has_no_pen_tip() {
    let svg = pen("");
    assert!(!svg.contains("<circle") && !svg.contains("<animateMotion"), "{svg}");
    assert!(svg.ends_with("</g></svg>"));
}

#[cfg(feature = "emoji")]
#[test]
fn the_pen_follows_the_quadratic_curves_of_a_truetype_glyph() {
    // Noto Emoji is TrueType: its outlines are quadratic curves, cut into short lines
    let svg = pen("😀");
    let d = strokes(&svg);
    assert!(!d.is_empty(), "{svg}");
    assert!(d.iter().map(|s| s.matches(" L").count()).sum::<usize>() > 20);
    assert!(svg.contains("<animateMotion"));
}
