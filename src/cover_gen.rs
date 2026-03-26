use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use rand::{Rng, RngExt, SeedableRng};
use rand::rngs::StdRng;
use resvg::usvg::{Options, Tree, fontdb};
use serde::Deserialize;
use tiny_skia::Pixmap;

static TEMPLATE: &str = include_str!("../assets/cover_template.svg");
static PALETTES_TOML: &str = include_str!("../assets/palettes.toml");
static FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/Lora/Lora-VariableFont_wght.ttf");
static FONT_ITALIC: &[u8] =
    include_bytes!("../assets/fonts/Lora/Lora-Italic-VariableFont_wght.ttf");

#[derive(Debug, Deserialize)]
struct PaletteFile {
    palette: Vec<Palette>,
}

#[derive(Debug, Deserialize)]
struct Palette {
    #[allow(dead_code)]
    name: String,
    bg: String,
    fg: String,
    acc: String,
    muted: String,
    grad_a: String,
    grad_b: String,
}

fn load_palettes() -> Vec<Palette> {
    let data: PaletteFile = toml::from_str(PALETTES_TOML).expect("failed to parse palettes.toml");
    data.palette
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn gradient_params(rng: &mut impl Rng, p: &Palette) -> [String; 6] {
    let angle = rng.random_range(0..360u32);
    let solid = rng.random_bool(0.4);

    let c1 = if rng.random_bool(0.5) { &p.grad_a } else { &p.bg };
    let c2 = if solid || rng.random_bool(0.5) {
        c1
    } else {
        &p.grad_b
    };

    let rad: f64 = (angle as f64).to_radians();
    let x1 = (50.0_f64 - 50.0 * rad.cos()).round() as u32;
    let y1 = (50.0_f64 - 50.0 * rad.sin()).round() as u32;
    let x2 = (50.0_f64 + 50.0 * rad.cos()).round() as u32;
    let y2 = (50.0_f64 + 50.0 * rad.sin()).round() as u32;

    [
        x1.to_string(),
        y1.to_string(),
        x2.to_string(),
        y2.to_string(),
        c1.to_owned(),
        c2.to_owned(),
    ]
}

fn wrap_title(title: &str) -> Vec<String> {
    let words: Vec<&str> = title.split_whitespace().collect();
    if words.is_empty() {
        return vec![];
    }

    if words.len() <= 2 {
        return vec![title.to_owned()];
    }

    let max_lines = 3;
    let char_px = 0.47;

    let mut lines: Vec<String> = Vec::new();
    let mut remaining = words.as_slice();

    for line_idx in 0..max_lines {
        if remaining.is_empty() {
            break;
        }

        let lines_left = max_lines - line_idx;
        let avg_chars: f64 =
            remaining.iter().map(|w| w.len() as f64).sum::<f64>() / remaining.len() as f64;
        let words_left = remaining.len();

        let target_px = if lines_left == 1 {
            340.0
        } else {
            let words_per_remaining = words_left as f64 / lines_left as f64;
            let avg_word_px = (avg_chars + 1.0) * char_px;
            (words_per_remaining * avg_word_px * 2.0).min(280.0)
        };

        let mut current: Vec<&str> = Vec::new();
        let mut current_px = 0.0;
        let mut consumed = 0;

        for (i, &word) in remaining.iter().enumerate() {
            let word_px = word.len() as f64 * char_px;
            let space_px = if current.is_empty() { 0.0 } else { 5.0 };

            if current_px + space_px + word_px > target_px && !current.is_empty() {
                break;
            }

            if !current.is_empty() {
                current_px += 5.0;
            }
            current.push(word);
            current_px += word_px;
            consumed = i + 1;
        }

        if current.is_empty() {
            current.push(remaining[0]);
            consumed = 1;
        }

        lines.push(current.join(" "));
        remaining = &remaining[consumed..];
    }

    if !remaining.is_empty()
        && let Some(last) = lines.last_mut()
    {
        last.push(' ');
        last.push_str(&remaining.join(" "));
    }

    lines
}

fn generate_shapes(rng: &mut impl Rng, acc: &str) -> [String; 4] {
    let shape_types: Vec<usize> = (0..4).map(|_| rng.random_range(0..4)).collect();
    let mut shapes = [const { String::new() }; 4];

    for (i, &shape) in shape_types.iter().enumerate() {
        let opacity = rng.random_range(20..=55) as f32 / 100.0;

        shapes[i] = match shape {
            0 => {
                let cx = rng.random_range(60..340u32);
                let cy = rng.random_range(40..320u32);
                let r = rng.random_range(40..130u32);
                format!(
                    r#"<circle cx="{cx}" cy="{cy}" r="{r}" fill="{acc}" opacity="{opacity:.2}"/>"#
                )
            }
            1 => {
                let cx = rng.random_range(60..340u32);
                let cy = rng.random_range(60..300u32);
                let w = rng.random_range(80..220u32);
                let h = rng.random_range(80..220u32);
                let rx = rng.random_range(0..40u32);
                let rot = rng.random_range(0..45u32);
                let x = cx.saturating_sub(w / 2);
                let y = cy.saturating_sub(h / 2);
                format!(
                    r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{acc}" opacity="{opacity:.2}" transform="rotate({rot} {cx} {cy})"/>"#
                )
            }
            2 => {
                let cx = rng.random_range(60..340u32);
                let cy = rng.random_range(60..320u32);
                let rx = rng.random_range(60..160u32);
                let ry = rng.random_range(30..80u32);
                let rot = rng.random_range(0..180u32);
                format!(
                    r#"<ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="{ry}" fill="{acc}" opacity="{opacity:.2}" transform="rotate({rot} {cx} {cy})"/>"#
                )
            }
            _ => {
                let n = rng.random_range(3..=6usize);
                let pts: Vec<String> = (0..n)
                    .map(|_| {
                        format!(
                            "{},{}",
                            rng.random_range(40..360u32),
                            rng.random_range(40..420u32)
                        )
                    })
                    .collect();
                let sw = rng.random_range(1..=3u32);
                let op = rng.random_range(40..=80) as f32 / 100.0;
                format!(
                    r#"<polygon points="{}" fill="none" stroke="{acc}" stroke-width="{sw}" opacity="{op:.2}"/>"#,
                    pts.join(" ")
                )
            }
        };
    }

    shapes
}

pub fn render_cover(title: &str, author: &str) -> anyhow::Result<Vec<u8>> {
    let palettes = load_palettes();
    let seed = {
        let mut hasher = DefaultHasher::new();
        title.hash(&mut hasher);
        '|'.hash(&mut hasher);
        author.hash(&mut hasher);
        hasher.finish()
    };
    let mut rng = StdRng::seed_from_u64(seed);
    let p = &palettes[rng.random_range(0..palettes.len())];

    let g = gradient_params(&mut rng, p);
    let shapes = generate_shapes(&mut rng, &p.acc);
    let lines = wrap_title(title);

     // Adaptive font size: find longest line, scale so it fits ~300px.
     let single_line = lines.len() == 1;
     let longest = lines.iter().map(|l| l.len()).max().unwrap_or(0);
     let max_px = longest as f64 * 0.47; // char width at 1px
     let font_size = if single_line {
         let scale = 300.0 / max_px;
         scale.round().clamp(20.0, 52.0) as u32
     } else {
         let scale = 300.0 / max_px;
         scale.round().clamp(20.0, 42.0) as u32
     };

    let line1 = lines.first().map(String::as_str).unwrap_or("");
    let line2 = lines.get(1).map(String::as_str).unwrap_or("");
    let line3 = lines.get(2).map(String::as_str).unwrap_or("");
    let n_lines = lines.len();
    let line2_vis = if n_lines > 1 { "visible" } else { "hidden" };
    let line3_vis = if n_lines > 2 { "visible" } else { "hidden" };

    // Vertical layout: line spacing = 1.1em relative to font size.
    let line_gap = font_size as f64 * 1.1;
    let title_top = 258.0;
    let after_title = title_top + line_gap * (n_lines as f64 - 1.0);
    let divider_y = (after_title + 28.0) as u32;
    let author_y = divider_y + 30;

    let svg = TEMPLATE
        .replace("{{grad_x1}}", &g[0])
        .replace("{{grad_y1}}", &g[1])
        .replace("{{grad_x2}}", &g[2])
        .replace("{{grad_y2}}", &g[3])
        .replace("{{grad_c1}}", &g[4])
        .replace("{{grad_c2}}", &g[5])
        .replace("{{shape1}}", &shapes[0])
        .replace("{{shape2}}", &shapes[1])
        .replace("{{shape3}}", &shapes[2])
        .replace("{{shape4}}", &shapes[3])
        .replace("{{title_size}}", &font_size.to_string())
        .replace("{{title_line1}}", &escape_xml(line1))
        .replace("{{title_line2}}", &escape_xml(line2))
        .replace("{{title_line3}}", &escape_xml(line3))
        .replace("{{line2_vis}}", line2_vis)
        .replace("{{line3_vis}}", line3_vis)
        .replace("{{divider_y}}", &divider_y.to_string())
        .replace("{{author_y}}", &author_y.to_string())
        .replace("{{fg}}", &p.fg)
        .replace("{{acc}}", &p.acc)
        .replace("{{muted}}", &p.muted)
        .replace("{{author}}", &escape_xml(author));

    let mut fontdb = fontdb::Database::new();
    fontdb.load_font_data(FONT_REGULAR.to_vec());
    fontdb.load_font_data(FONT_ITALIC.to_vec());

    let opts = Options {
        fontdb: Arc::new(fontdb),
        ..Options::default()
    };
    let tree = Tree::from_str(&svg, &opts)?;

    let size = tree.size();
    let width = size.width() as u32;
    let height = size.height() as u32;

    let mut pixmap = Pixmap::new(width, height)
        .ok_or_else(|| anyhow::anyhow!("failed to create pixmap ({width}x{height})"))?;

    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    pixmap
        .encode_png()
        .map_err(|e| anyhow::anyhow!("failed to encode PNG: {e}"))
}

#[cfg(test)]
fn save_test_cover(name: &str, png: &[u8]) {
    if std::env::var("SAVE_COVERS").is_ok() {
        std::fs::create_dir_all("test_outputs").unwrap();
        std::fs::write(format!("test_outputs/{name}.png"), png).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("AT&T"), "AT&amp;T");
        assert_eq!(escape_xml("<tag>"), "&lt;tag&gt;");
        assert_eq!(escape_xml(r#"He said "hi""#), "He said &quot;hi&quot;");
    }

    #[test]
    fn test_palettes_load() {
        let palettes = load_palettes();
        assert!(palettes.len() >= 20);
        assert!(!palettes[0].bg.is_empty());
        assert!(!palettes[0].grad_a.is_empty());
    }

    #[test]
    fn test_wrap_short_title() {
        let lines = wrap_title("Dune");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "Dune");
    }

    #[test]
    fn test_wrap_long_title() {
        let lines =
            wrap_title("The Return of the King: Being the Third Part of The Lord of the Rings");
        assert!(lines.len() >= 2);
        assert!(lines.len() <= 3);
        for line in &lines {
            assert!(
                line.len() <= 45,
                "line too long ({} chars): {}",
                line.len(),
                line
            );
        }
    }

    #[test]
    fn test_render_cover() {
        let png = render_cover("Dune", "Frank Herbert").unwrap();
        assert!(!png.is_empty());
        assert_eq!(&png[0..4], &[0x89, b'P', b'N', b'G']);
        save_test_cover("test_cover_dune", &png);
    }

    #[test]
    fn test_render_long_title() {
        let png = render_cover(
            "The Return of the King: Being the Third Part of The Lord of the Rings",
            "J.R.R. Tolkien",
        )
        .unwrap();
        assert!(!png.is_empty());
        save_test_cover("test_cover_tolkien", &png);
    }

    #[test]
    fn test_render_is_deterministic() {
        let a = render_cover("Dune", "Frank Herbert").unwrap();
        let b = render_cover("Dune", "Frank Herbert").unwrap();
        assert_eq!(a, b);

        let c = render_cover("Dune", "Brian Herbert").unwrap();
        assert_ne!(a, c);
        save_test_cover("test_cover_dune_brian", &c);
    }
}
