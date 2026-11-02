//! Coordinate-only map geometry export. Does not construct a simulation or solve flow.
use simulation::map::MapDefinition;
use std::{fmt::Write, path::PathBuf};

#[path = "support/map_svg.rs"]
mod map_svg;

const HELP: &str = "Export map geometry as SVG (world units, Y points down).\n\
Usage: map_render --rect X Y WIDTH HEIGHT --output FILE.svg [--map heart|small-world] [--grid STEP] [--labels]\n\
Default map: heart. Width, height and grid step must be positive and finite.\n\
Example: map_render --rect -48 -150 288 390 --grid 16 --labels --output heart.svg";

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn contains(self, p: [f64; 2]) -> bool {
        p[0] >= self.x && p[0] <= self.x + self.w && p[1] >= self.y && p[1] <= self.y + self.h
    }
}

#[derive(Debug)]
struct Options {
    rect: Rect,
    output: PathBuf,
    map: String,
    grid: Option<f64>,
    labels: bool,
}

fn number(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<f64, String> {
    args.next()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("{flag}: expected a finite number"))
}

fn parse(args: impl IntoIterator<Item = String>) -> Result<Option<Options>, String> {
    let mut args = args.into_iter();
    let (mut rect, mut output, mut grid) = (None, None, None);
    let mut map = String::from("heart");
    let mut labels = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => return Ok(None),
            "--rect" => {
                let r = Rect {
                    x: number(&mut args, "--rect X")?,
                    y: number(&mut args, "--rect Y")?,
                    w: number(&mut args, "--rect WIDTH")?,
                    h: number(&mut args, "--rect HEIGHT")?,
                };
                if r.w <= 0.
                    || r.h <= 0.
                    || !(r.x + r.w).is_finite()
                    || !(r.y + r.h).is_finite()
                    || r.x + r.w <= r.x
                    || r.y + r.h <= r.y
                {
                    return Err("--rect: width and height must be positive, finite and representable at X/Y".into());
                }
                rect = Some(r);
            }
            "--output" => {
                output = Some(PathBuf::from(
                    args.next()
                        .filter(|v| !v.is_empty() && !v.starts_with("--"))
                        .ok_or("--output: expected a file path")?,
                ));
            }
            "--map" => {
                map = args.next().ok_or("--map: expected heart or small-world")?;
                if map != "heart" && map != "small-world" {
                    return Err("--map: expected heart or small-world".into());
                }
            }
            "--grid" => {
                let step = number(&mut args, "--grid")?;
                if step <= 0. {
                    return Err("--grid: step must be positive".into());
                }
                grid = Some(step);
            }
            "--labels" => labels = true,
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    let rect = rect.ok_or("missing --rect X Y WIDTH HEIGHT")?;
    let output = output.ok_or("missing --output FILE.svg")?;
    if let Some(step) = grid {
        let count = (rect.w / step).floor() + (rect.h / step).floor() + 2.;
        if count > 200.
            || step / rect.w.max(rect.h) * 1200. < 12.
            || !(rect.x / step).is_finite()
            || !(rect.y / step).is_finite()
        {
            return Err("--grid: too dense; increase STEP (at least 12 pixels between lines, at most 200 lines)".into());
        }
    }
    Ok(Some(Options {
        rect,
        output,
        map,
        grid,
        labels,
    }))
}

// Clip one centerline segment to the rectangle, including crossing segments with both ends outside.
fn clipped_midpoint(a: [f32; 2], b: [f32; 2], r: Rect) -> Option<[f64; 2]> {
    let a = a.map(f64::from);
    let b = b.map(f64::from);
    let d = [b[0] - a[0], b[1] - a[1]];
    let (mut enter, mut leave) = (0_f64, 1_f64);
    for (axis, low, high) in [(0, r.x, r.x + r.w), (1, r.y, r.y + r.h)] {
        if d[axis] == 0. {
            if a[axis] < low || a[axis] > high {
                return None;
            }
        } else {
            let t0 = (low - a[axis]) / d[axis];
            let t1 = (high - a[axis]) / d[axis];
            enter = enter.max(t0.min(t1));
            leave = leave.min(t0.max(t1));
            if enter > leave {
                return None;
            }
        }
    }
    let t = (enter + leave) * 0.5;
    Some([a[0] + d[0] * t, a[1] + d[1] * t])
}

fn visible_anchor(points: &[[f32; 2]], r: Rect) -> Option<[f64; 2]> {
    points
        .windows(2)
        .find_map(|p| clipped_midpoint(p[0], p[1], r))
}

fn text(svg: &mut String, p: [f64; 2], label: &str, font: f64) {
    write!(svg, r##"<text x="{}" y="{}" font-size="{font}" fill="#fff" font-family="sans-serif">{}</text>"##,
        p[0], p[1], map_svg::escape(label)).unwrap();
}

fn render(map: &MapDefinition, options: &Options) -> String {
    let r = options.rect;
    let scale = 1200. / r.w.max(r.h);
    let font = 13. / scale;
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" overflow="hidden" width="{}" height="{}" viewBox="{} {} {} {}"><title>{} map geometry; world units; Y down</title><rect x="{}" y="{}" width="{}" height="{}" fill="#172333"/>"##,
        r.w * scale,
        r.h * scale,
        r.x,
        r.y,
        r.w,
        r.h,
        options.map,
        r.x,
        r.y,
        r.w,
        r.h
    );
    map_svg::write_geometry(&mut svg, map, |i| {
        format!("edge {i}, D{}", map.edges[i].half_width * 2.)
    });
    if let Some(step) = options.grid {
        for (axis, start, span) in [(0, r.x, r.w), (1, r.y, r.h)] {
            let first = (start / step).ceil() * step;
            let count = ((start + span - first) / step).floor() as i32;
            for i in 0..=count {
                let v = first + f64::from(i) * step;
                let (a, b, label) = if axis == 0 {
                    ([v, r.y], [v, r.y + r.h], [v + font * 0.2, r.y + font * 1.2])
                } else {
                    ([r.x, v], [r.x + r.w, v], [r.x + font * 0.2, v - font * 0.2])
                };
                write!(svg, r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#566474" stroke-width="{}"/>"##,
                    a[0], a[1], b[0], b[1], 0.7 / scale).unwrap();
                text(&mut svg, label, &format!("{v}"), font);
            }
        }
    }
    if options.labels {
        for (i, edge) in map.edges.iter().enumerate() {
            if let Some(anchor) = visible_anchor(&edge.centerline, r) {
                text(
                    &mut svg,
                    anchor,
                    &format!("{i} D{}", edge.half_width * 2.),
                    font,
                );
            }
        }
        for chamber in &map.chambers {
            let bounds = chamber.bounds();
            let center = [
                (f64::from(bounds[0][0]) + f64::from(bounds[1][0])) / 2.,
                (f64::from(bounds[0][1]) + f64::from(bounds[1][1])) / 2.,
            ];
            let mut closed = chamber.boundary.clone();
            closed.push(closed[0]);
            let viewport_center = [r.x + r.w / 2., r.y + r.h / 2.];
            let anchor = if r.contains(center) {
                Some(center)
            } else if let Some(anchor) = visible_anchor(&closed, r) {
                Some(anchor)
            } else if chamber.signed_distance(rapier2d::prelude::Vector::new(
                viewport_center[0] as f32,
                viewport_center[1] as f32,
            )) <= 0.
            {
                // The entire selected rectangle can lie inside a chamber.
                Some(viewport_center)
            } else {
                None
            };
            if let Some(anchor) = anchor {
                text(&mut svg, anchor, &chamber.name, font);
            }
        }
    }
    svg.push_str("</svg>\n");
    svg
}

fn run() -> Result<(), String> {
    let Some(options) = parse(std::env::args().skip(1))? else {
        println!("{HELP}");
        return Ok(());
    };
    let map = match options.map.as_str() {
        "small-world" => MapDefinition::small_world(),
        _ => MapDefinition::heart_preview(),
    };
    std::fs::write(&options.output, render(&map, &options))
        .map_err(|e| format!("cannot write {}: {e}", options.output.display()))?;
    println!("Wrote {}", options.output.display());
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("map_render: {error}\nUse --help for usage.");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(args: &str) -> Result<Option<Options>, String> {
        parse(args.split_whitespace().map(str::to_owned))
    }

    #[test]
    fn accepts_negative_coordinates_and_rejects_invalid_arguments() {
        let opts = options("--rect -7 -9 20 30 --output a.svg")
            .unwrap()
            .unwrap();
        assert_eq!(
            opts.rect,
            Rect {
                x: -7.,
                y: -9.,
                w: 20.,
                h: 30.
            }
        );
        for args in [
            "--output a.svg",
            "--rect 0 0 0 1 --output a.svg",
            "--rect NaN 0 1 1 --output a.svg",
            "--rect 0 0 1 inf --output a.svg",
            "--rect 0 0 1 1",
            "--rect 0 0 20 20 --grid 0.001 --output a.svg",
            "--rect 0 0 1 1 --map unknown --output a.svg",
        ] {
            assert!(options(args).is_err(), "{args}");
        }
        assert!(options("--help").unwrap().is_none());
    }

    #[test]
    fn serializes_crossing_geometry_visible_labels_and_world_aligned_grid() {
        let mut map = MapDefinition::small_world();
        map.chambers.clear();
        map.edges.truncate(1);
        map.edges[0].centerline = vec![[-100., 0.], [100., 0.]];
        map.edges[0].half_width = 3.;
        let opts = options("--rect -7 -9 20 30 --grid 5 --labels --output a.svg")
            .unwrap()
            .unwrap();
        let svg = render(&map, &opts);
        assert!(svg.contains("points=\"-100,0 100,0\""));
        assert!(svg.contains("stroke-width=\"6\""));
        assert!(svg.contains("overflow=\"hidden\""));
        assert!(svg.contains("<text x=\"3") && svg.contains(">0 D6</text>"));
        assert!(svg.contains("<line x1=\"-5\" y1=\"-9\" x2=\"-5\""));
        assert!(!svg.contains("<line x1=\"-7\" y1=\"-9\" x2=\"-7\""));
        assert!(visible_anchor(&[[-100., 50.], [100., 50.]], opts.rect).is_none());
    }

    #[test]
    fn escapes_chamber_names_in_svg_text() {
        let mut map = MapDefinition::heart_preview();
        map.chambers[0].name = "A<&\"B".into();
        let opts = options("--rect -1000 -1500 2000 3000 --labels --output a.svg")
            .unwrap()
            .unwrap();
        assert!(render(&map, &opts).contains("A&lt;&amp;&quot;B"));
    }
}
