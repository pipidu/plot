use sciplot_core::Dash;

use crate::scene::{dash_intervals, HAlign, Prim, Scene, VAlign};

pub const SVG_FONT: &str = "Microsoft YaHei, Noto Sans SC, Droid Sans Fallback, WenQuanYi Micro Hei, sans-serif";

pub fn to_svg(scene: &Scene) -> String {
    let w = scene.width.max(1.0);
    let h = scene.height.max(1.0);
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n"
    );
    for prim in &scene.primitives {
        match prim {
            Prim::Line { x1, y1, x2, y2, color, width, dash } => {
                out.push_str(&format!(
                    "<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"{}\" stroke-width=\"{width}\"{}/>\n",
                    hex(*color),
                    dash_attr(*dash, *width)
                ));
            }
            Prim::Polyline { pts, color, width, dash } => {
                if pts.len() < 2 {
                    continue;
                }
                let points = pts.iter().map(|(x, y)| format!("{x},{y}")).collect::<Vec<_>>().join(" ");
                out.push_str(&format!(
                    "<polyline fill=\"none\" stroke=\"{}\" stroke-width=\"{width}\" stroke-linejoin=\"round\" stroke-linecap=\"round\" points=\"{points}\"{}/>\n",
                    hex(*color),
                    dash_attr(*dash, *width)
                ));
            }
            Prim::Polygon { pts, fill } => {
                if pts.len() < 3 {
                    continue;
                }
                let points = pts.iter().map(|(x, y)| format!("{x},{y}")).collect::<Vec<_>>().join(" ");
                out.push_str(&format!("<polygon points=\"{points}\" fill=\"{}\"/>\n", hex(*fill)));
            }
            Prim::Rect { x, y, w, h, fill, stroke } => {
                let fill = fill.map(hex).unwrap_or_else(|| "none".to_string());
                let (stroke, sw) = stroke.map(|(c, w)| (hex(c), w)).unwrap_or_else(|| ("none".to_string(), 0.0));
                out.push_str(&format!(
                    "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\"/>\n"
                ));
            }
            Prim::Circle { x, y, r, fill, stroke } => {
                let fill = fill.map(hex).unwrap_or_else(|| "none".to_string());
                let (stroke, sw) = stroke.map(|(c, w)| (hex(c), w)).unwrap_or_else(|| ("none".to_string(), 0.0));
                out.push_str(&format!(
                    "<circle cx=\"{x}\" cy=\"{y}\" r=\"{r}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\"/>\n"
                ));
            }
            Prim::Triangle { a, b, c, fill } => {
                out.push_str(&format!(
                    "<polygon points=\"{},{} {},{} {},{}\" fill=\"{}\"/>\n",
                    a.0, a.1, b.0, b.1, c.0, c.1, hex(*fill)
                ));
            }
            Prim::Text { x, y, text, size, color, halign, valign, vertical } => {
                if *vertical {
                    let chars: Vec<char> = text.chars().collect();
                    let start = match valign {
                        VAlign::Top => *y,
                        VAlign::Center => *y - chars.len() as f32 * *size * 0.5,
                        VAlign::Bottom => *y - chars.len() as f32 * *size,
                    };
                    for (index, ch) in chars.into_iter().enumerate() {
                        let yy = start + (index as f32 + 0.85) * *size;
                        out.push_str(&format!(
                            "<text x=\"{x}\" y=\"{yy}\" fill=\"{}\" font-size=\"{size}\" font-family=\"{SVG_FONT}\" text-anchor=\"middle\">{ch}</text>\n",
                            hex(*color)
                        ));
                    }
                } else {
                    let anchor = match halign {
                        HAlign::Left => "start",
                        HAlign::Center => "middle",
                        HAlign::Right => "end",
                    };
                    let baseline = match valign {
                        VAlign::Top => "hanging",
                        VAlign::Center => "middle",
                        VAlign::Bottom => "auto",
                    };
                    out.push_str(&format!(
                        "<text x=\"{x}\" y=\"{y}\" fill=\"{}\" font-size=\"{size}\" font-family=\"{SVG_FONT}\" text-anchor=\"{anchor}\" dominant-baseline=\"{baseline}\">{}</text>\n",
                        hex(*color),
                        escape(text)
                    ));
                }
            }
        }
    }
    out.push_str("</svg>\n");
    out
}

fn dash_attr(dash: Dash, width: f32) -> String {
    match dash_intervals(dash, width) {
        Some(parts) => {
            let list = parts.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(" ");
            format!(" stroke-dasharray=\"{list}\"")
        }
        None => String::new(),
    }
}

fn hex(color: [u8; 4]) -> String {
    if color[3] == 255 {
        format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2])
    } else {
        format!("rgba({},{},{},{:.3})", color[0], color[1], color[2], color[3] as f32 / 255.0)
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
