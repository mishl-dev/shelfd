use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};

use rand::rngs::StdRng;
use rand::{Rng, RngExt, SeedableRng};
use resvg::usvg::{Options, Tree, fontdb};
use serde::Deserialize;
use tiny_skia::Pixmap;

mod templates;

use templates::{TemplateKind, render_template};

static PALETTES_TOML: &str = include_str!("../../assets/palettes.toml");
static FONT_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Lora/Lora-VariableFont_wght.ttf");
static FONT_ITALIC: &[u8] =
    include_bytes!("../../assets/fonts/Lora/Lora-Italic-VariableFont_wght.ttf");

fn shared_fontdb() -> Arc<fontdb::Database> {
    static DB: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_font_data(FONT_REGULAR.to_vec());
        db.load_font_data(FONT_ITALIC.to_vec());
        Arc::new(db)
    })
    .clone()
}

fn shared_palettes() -> &'static [Palette] {
    static PALETTES: OnceLock<Vec<Palette>> = OnceLock::new();
    PALETTES.get_or_init(|| {
        let data: PaletteFile =
            toml::from_str(PALETTES_TOML).expect("failed to parse palettes.toml");
        data.palette
    })
}

const WIDTH: u32 = 400;
const HEIGHT: u32 = 600;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextAlign {
    Left,
    Center,
}

#[derive(Clone, Copy, Debug)]
struct TitleSpec {
    x: f32,
    y: f32,
    max_width: f32,
    max_height: f32,
    align: TextAlign,
    center_block: bool,
    min_size: u32,
    max_size: u32,
    single_line_max: u32,
    letter_spacing: f32,
}

#[derive(Clone, Copy, Debug)]
struct TitleMetrics {
    font_size: u32,
    last_baseline: f32,
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn wrap_title(title: &str) -> Vec<String> {
    let words: Vec<&str> = title.split_whitespace().collect();
    if words.is_empty() {
        return vec![];
    }

    if words.len() <= 2 {
        return vec![title.to_owned()];
    }

    let max_lines = 5;
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

fn initial_letter(title: &str) -> String {
    title
        .chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "•".to_string())
}

fn estimate_line_units(line: &str) -> (f32, usize) {
    let mut units: f32 = 0.0;
    let mut count = 0;

    for ch in line.chars() {
        units += match ch {
            ' ' => 0.34,
            'A'..='Z' => 0.72,
            'a'..='z' => 0.58,
            '0'..='9' => 0.56,
            ':' | ';' | ',' | '.' | '\'' | '"' => 0.30,
            '-' | '–' | '—' => 0.38,
            '&' => 0.72,
            _ => 0.62,
        };
        count += 1;
    }

    (units.max(1.0), count)
}

fn line_fits_width(line: &str, font_size: u32, max_width: f32, letter_spacing: f32) -> bool {
    let target_width = (max_width - 28.0).max(max_width * 0.78);
    let (units, glyph_count) = estimate_line_units(line);
    let spacing = letter_spacing.max(0.0) * glyph_count.saturating_sub(1) as f32;
    let estimated_width = units * font_size as f32 + spacing;
    estimated_width <= target_width
}

fn render_title(lines: &[String], spec: TitleSpec, fill: &str) -> (String, TitleMetrics) {
    let line_count = lines.len().max(1) as f32;
    let size_from_height = (spec.max_height / (1.0 + 1.12 * (line_count - 1.0))).floor() as u32;
    let max_candidate = if lines.len() <= 1 {
        size_from_height
            .min(spec.single_line_max)
            .min(spec.max_size)
    } else {
        size_from_height.min(spec.max_size)
    };
    let mut font_size = max_candidate.max(spec.min_size);

    while font_size > spec.min_size
        && !lines
            .iter()
            .all(|line| line_fits_width(line, font_size, spec.max_width, spec.letter_spacing))
    {
        font_size -= 1;
    }

    let font_size = font_size.clamp(spec.min_size, max_candidate.max(spec.min_size));
    let line_gap = font_size as f32 * 1.12;
    let anchor = match spec.align {
        TextAlign::Left => "start",
        TextAlign::Center => "middle",
    };
    let first_baseline = if spec.center_block {
        spec.y - (line_gap * (lines.len().saturating_sub(1) as f32)) / 2.0
    } else {
        spec.y
    };

    let mut block = format!(
        r#"<text x="{:.1}" y="{:.1}" font-family="'Lora','Georgia',serif" font-size="{}" font-weight="700" fill="{}" letter-spacing="{:.1}" text-anchor="{}">"#,
        spec.x, first_baseline, font_size, fill, spec.letter_spacing, anchor
    );

    for (idx, line) in lines.iter().enumerate() {
        let dy = if idx == 0 { 0.0 } else { line_gap };
        block.push_str(&format!(
            r#"<tspan x="{:.1}" dy="{:.1}">{}</tspan>"#,
            spec.x,
            dy,
            escape_xml(line)
        ));
    }
    block.push_str("</text>");

    (
        block,
        TitleMetrics {
            font_size,
            last_baseline: first_baseline + line_gap * (lines.len().saturating_sub(1) as f32),
        },
    )
}

fn author_text(author: &str, x: f32, y: f32, align: TextAlign, fill: &str, opacity: f32) -> String {
    let anchor = match align {
        TextAlign::Left => "start",
        TextAlign::Center => "middle",
    };
    format!(
        r#"<text x="{x:.1}" y="{y:.1}" font-family="'Lora','Georgia',serif" font-size="20" font-style="italic" fill="{fill}" opacity="{opacity:.2}" text-anchor="{anchor}">{}</text>"#,
        escape_xml(author)
    )
}

fn wave_path(y: f32, amplitude: f32, width: f32) -> String {
    format!(
        "M -20,{y:.1} Q 80,{q1:.1} 200,{y:.1} Q 320,{q2:.1} {width:.1},{y:.1}",
        q1 = y - amplitude,
        q2 = y + amplitude
    )
}

fn bezier_blob_at(
    rng: &mut impl Rng,
    cx: f32,
    cy: f32,
    base_r: f32,
    acc: &str,
    opacity: f32,
) -> String {
    let n = rng.random_range(5..=7usize);
    let pts: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let angle = (i as f32 / n as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            let radius = base_r * rng.random_range(0.6f32..1.18f32);
            (cx + radius * angle.cos(), cy + radius * angle.sin())
        })
        .collect();

    let mid = |a: (f32, f32), b: (f32, f32)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let start = mid(pts[n - 1], pts[0]);
    let mut d = format!("M {:.1},{:.1}", start.0, start.1);
    for i in 0..n {
        let ctrl = pts[i];
        let end = mid(pts[i], pts[(i + 1) % n]);
        d.push_str(&format!(
            " Q {:.1},{:.1} {:.1},{:.1}",
            ctrl.0, ctrl.1, end.0, end.1
        ));
    }
    d.push_str(" Z");

    format!(r#"<path d="{d}" fill="{acc}" opacity="{opacity:.2}"/>"#)
}

fn svg_document(defs: &str, body: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 {WIDTH} {HEIGHT}" width="{WIDTH}" height="{HEIGHT}"><defs>{defs}</defs>{body}</svg>"#
    )
}

fn render_svg_pixmap(svg: &str) -> anyhow::Result<Pixmap> {
    let opts = Options {
        fontdb: shared_fontdb(),
        ..Options::default()
    };
    let tree = Tree::from_str(svg, &opts)?;

    let size = tree.size();
    let width = size.width() as u32;
    let height = size.height() as u32;
    let mut pixmap = Pixmap::new(width, height)
        .ok_or_else(|| anyhow::anyhow!("failed to create pixmap ({width}x{height})"))?;

    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    Ok(pixmap)
}

fn render_svg(svg: &str) -> anyhow::Result<Vec<u8>> {
    let pixmap = render_svg_pixmap(svg)?;
    pixmap
        .encode_png()
        .map_err(|e| anyhow::anyhow!("failed to encode PNG: {e}"))
}

pub fn render_cover(title: &str, author: &str) -> anyhow::Result<Vec<u8>> {
    let palettes = shared_palettes();
    let seed = {
        let mut hasher = DefaultHasher::new();
        title.hash(&mut hasher);
        '|'.hash(&mut hasher);
        author.hash(&mut hasher);
        hasher.finish()
    };
    let mut rng = StdRng::seed_from_u64(seed);
    let p = &palettes[rng.random_range(0..palettes.len())];
    let template = TemplateKind::ALL[rng.random_range(0..TemplateKind::ALL.len())];
    let svg = render_template(template, title, author, p, &mut rng);
    render_svg(&svg)
}

#[cfg(test)]
fn save_test_cover(name: &str, png: &[u8]) {
    if std::env::var("SAVE_COVERS").is_ok() {
        std::fs::create_dir_all("test_outputs").unwrap();
        std::fs::write(format!("test_outputs/{name}.png"), png).unwrap();
    }
}

#[cfg(test)]
fn save_template_cover(name: &str, png: &[u8]) {
    if std::env::var("SAVE_COVERS").is_ok() {
        std::fs::create_dir_all("test_outputs/templates").unwrap();
        std::fs::write(format!("test_outputs/templates/{name}.png"), png).unwrap();
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
        let palettes = shared_palettes();
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
        assert!(lines.len() <= 5);
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
    fn test_template_svg_generation() {
        let palettes = shared_palettes();
        let palette = &palettes[0];

        for (idx, template) in TemplateKind::ALL.into_iter().enumerate() {
            let mut rng = StdRng::seed_from_u64((idx + 1) as u64);
            let svg = render_template(template, "Dune Messiah", "Frank Herbert", palette, &mut rng);
            assert!(svg.contains("<svg"));
            assert!(svg.contains("Dune Messiah"));
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

    #[test]
    fn test_generate_all_template_samples() {
        let palettes = shared_palettes();

        for (idx, template) in TemplateKind::ALL.into_iter().enumerate() {
            let mut rng = StdRng::seed_from_u64(10_000 + idx as u64);
            let palette = &palettes[(idx * 3) % palettes.len()];
            let svg = render_template(template, "Dune", "Frank Herbert", palette, &mut rng);
            let png = render_svg(&svg).unwrap();
            assert!(!png.is_empty());
            save_template_cover(&format!("template_{:02}", idx + 1), &png);
        }
    }

    #[test]
    fn test_generate_all_template_samples_long_title() {
        let palettes = shared_palettes();
        let title = "The Return of the King: Being the Third Part of The Lord of the Rings";
        let author = "J.R.R. Tolkien";

        for (idx, template) in TemplateKind::ALL.into_iter().enumerate() {
            let mut rng = StdRng::seed_from_u64(20_000 + idx as u64);
            let palette = &palettes[(idx * 5 + 1) % palettes.len()];
            let svg = render_template(template, title, author, palette, &mut rng);
            let png = render_svg(&svg).unwrap();
            assert!(!png.is_empty());
            save_template_cover(&format!("template_{:02}_long", idx + 1), &png);
        }
    }
}
