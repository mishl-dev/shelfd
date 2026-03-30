use rand::{Rng, RngExt};

use super::{
    author_text, bezier_blob_at, escape_xml, grain_overlay, initial_letter, render_title,
    svg_document, wave_path, Palette, TextAlign, TitleSpec, HEIGHT, WIDTH,
};

#[derive(Clone, Copy, Debug)]
pub(super) enum TemplateKind {
    MinimalDark,
    OffCentreCircles,
    SteppedBars,
    GiantInitial,
    OrganicInkBlob,
    CornerOrnaments,
    WaveStack,
    HalftoneDots,
    OverlappingCircles,
    StackedChevrons,
    LayeredObliques,
}

impl TemplateKind {
    pub(super) const ALL: [Self; 11] = [
        Self::MinimalDark,
        Self::OffCentreCircles,
        Self::SteppedBars,
        Self::GiantInitial,
        Self::OrganicInkBlob,
        Self::CornerOrnaments,
        Self::WaveStack,
        Self::HalftoneDots,
        Self::OverlappingCircles,
        Self::StackedChevrons,
        Self::LayeredObliques,
    ];
}

pub(super) fn render_template(
    template: TemplateKind,
    title: &str,
    author: &str,
    p: &Palette,
    rng: &mut impl Rng,
) -> String {
    let lines = super::wrap_title(title);

    match template {
        TemplateKind::MinimalDark => {
            let defs = format!(
                r#"<linearGradient id="bg" x1="{:.0}%" y1="0%" x2="{:.0}%" y2="100%"><stop offset="0%" stop-color="{}"/><stop offset="100%" stop-color="{}"/></linearGradient>"#,
                rng.random_range(10..=40),
                rng.random_range(0..=15),
                p.grad_a,
                p.bg
            );
            let (title_block, metrics) = render_title(
                &lines,
                TitleSpec {
                    x: 36.0,
                    y: 320.0,
                    max_width: 320.0,
                    max_height: 150.0,
                    align: TextAlign::Left,
                    center_block: false,
                    min_size: 22,
                    max_size: 74,
                    single_line_max: 86,
                    letter_spacing: 6.0,
                },
                &p.fg,
            );
            let divider_y = metrics.last_baseline + metrics.font_size as f32 * 0.28;
            let body = format!(
                r#"<rect width="{WIDTH}" height="{HEIGHT}" fill="url(#bg)"/><rect width="8" height="{HEIGHT}" fill="{acc}"/><line x1="36" y1="230" x2="{rule_end}" y2="230" stroke="{muted}" stroke-width="1.6"/>{title}<line x1="36" y1="{divider_y:.1}" x2="364" y2="{divider_y:.1}" stroke="{acc}" stroke-width="1.2" opacity="0.70"/>{author}{grain}"#,
                acc = p.acc,
                muted = p.muted,
                rule_end = rng.random_range(126..=196),
                title = title_block,
                author = author_text(author, 36.0, 576.0, TextAlign::Left, &p.fg, 0.58),
                grain = grain_overlay(0.07)
            );
            svg_document(&defs, &body)
        }
        TemplateKind::OffCentreCircles => {
            let defs = format!(
                r#"<linearGradient id="bg" x1="0%" y1="0%" x2="0%" y2="100%"><stop offset="0%" stop-color="{}"/><stop offset="100%" stop-color="{}"/></linearGradient>"#,
                p.grad_a, p.grad_b
            );
            let cx = rng.random_range(284.0..340.0);
            let cy = rng.random_range(104.0..168.0);
            let base = rng.random_range(112.0..152.0);
            let (title_block, metrics) = render_title(
                &lines,
                TitleSpec {
                    x: 36.0,
                    y: 340.0,
                    max_width: 290.0,
                    max_height: 150.0,
                    align: TextAlign::Left,
                    center_block: false,
                    min_size: 22,
                    max_size: 70,
                    single_line_max: 84,
                    letter_spacing: 5.0,
                },
                &p.fg,
            );
            let divider_y = metrics.last_baseline + metrics.font_size as f32 * 0.26;
            let body = format!(
                r#"<rect width="{WIDTH}" height="{HEIGHT}" fill="url(#bg)"/><circle cx="{cx:.1}" cy="{cy:.1}" r="{r1:.1}" fill="{acc}" opacity="0.08"/><circle cx="{cx:.1}" cy="{cy:.1}" r="{r2:.1}" fill="{acc}" opacity="0.11"/><circle cx="{cx:.1}" cy="{cy:.1}" r="{r3:.1}" fill="{acc}" opacity="0.18"/>{title}<line x1="36" y1="{divider_y:.1}" x2="{line_end}" y2="{divider_y:.1}" stroke="{acc}" stroke-width="1.6" opacity="0.80"/>{author}{grain}"#,
                acc = p.acc,
                r1 = base,
                r2 = base * 0.68,
                r3 = base * 0.36,
                title = title_block,
                line_end = rng.random_range(258..=326),
                author = author_text(author, 36.0, 574.0, TextAlign::Left, &p.fg, 0.62),
                grain = grain_overlay(0.05)
            );
            svg_document(&defs, &body)
        }
        TemplateKind::SteppedBars => {
            let start_y = rng.random_range(52.0..72.0);
            let bar_w = rng.random_range(250.0..320.0);
            let (title_block, metrics) = render_title(
                &lines,
                TitleSpec {
                    x: 30.0,
                    y: 310.0,
                    max_width: 330.0,
                    max_height: 150.0,
                    align: TextAlign::Left,
                    center_block: false,
                    min_size: 22,
                    max_size: 78,
                    single_line_max: 88,
                    letter_spacing: 4.0,
                },
                &p.fg,
            );
            let divider_y = metrics.last_baseline + metrics.font_size as f32 * 0.28;
            let body = format!(
                r#"<rect width="{WIDTH}" height="{HEIGHT}" fill="{bg}"/><rect x="0" y="{y1:.1}" width="{w1:.1}" height="40" fill="{acc}" opacity="0.76"/><rect x="0" y="{y2:.1}" width="{w2:.1}" height="24" fill="{acc}" opacity="0.52"/><rect x="0" y="{y3:.1}" width="{w3:.1}" height="14" fill="{acc}" opacity="0.32"/><rect x="0" y="{y4:.1}" width="{w4:.1}" height="8" fill="{acc}" opacity="0.18"/>{title}<line x1="30" y1="{divider_y:.1}" x2="370" y2="{divider_y:.1}" stroke="{acc}" stroke-width="1.3" opacity="0.55"/>{author}{grain}"#,
                bg = p.bg,
                acc = p.acc,
                y1 = start_y,
                y2 = start_y + 44.0,
                y3 = start_y + 74.0,
                y4 = start_y + 96.0,
                w1 = bar_w,
                w2 = bar_w * 0.72,
                w3 = bar_w * 0.46,
                w4 = bar_w * 0.28,
                title = title_block,
                author = author_text(author, 30.0, 574.0, TextAlign::Left, &p.fg, 0.58),
                grain = grain_overlay(0.05)
            );
            svg_document("", &body)
        }
        TemplateKind::GiantInitial => {
            let initial = escape_xml(&initial_letter(title));
            let (title_block, metrics) = render_title(
                &lines,
                TitleSpec {
                    x: 40.0,
                    y: 324.0,
                    max_width: 310.0,
                    max_height: 150.0,
                    align: TextAlign::Left,
                    center_block: false,
                    min_size: 22,
                    max_size: 72,
                    single_line_max: 82,
                    letter_spacing: 5.0,
                },
                &p.fg,
            );
            let divider_y = metrics.last_baseline + metrics.font_size as f32 * 0.26;
            let body = format!(
                r#"<rect width="{WIDTH}" height="{HEIGHT}" fill="{bg}"/><text x="-12" y="458" font-family="'Lora','Georgia',serif" font-size="{initial_size}" font-weight="700" fill="{acc}" opacity="0.10">{initial}</text><line x1="40" y1="248" x2="{rule_end}" y2="248" stroke="{muted}" stroke-width="1.4"/>{title}<line x1="40" y1="{divider_y:.1}" x2="360" y2="{divider_y:.1}" stroke="{acc}" stroke-width="1.2" opacity="0.64"/>{author}{grain}"#,
                bg = p.grad_a,
                acc = p.acc,
                muted = p.muted,
                initial_size = rng.random_range(320..=380),
                rule_end = rng.random_range(118..=184),
                title = title_block,
                author = author_text(author, 40.0, 576.0, TextAlign::Left, &p.fg, 0.62),
                grain = grain_overlay(0.04)
            );
            svg_document("", &body)
        }
        TemplateKind::OrganicInkBlob => {
            let blob1_r = rng.random_range(132.0..160.0);
            let blob2_r = rng.random_range(112.0..140.0);
            let blob1 = bezier_blob_at(rng, 182.0, 150.0, blob1_r, &p.acc, 0.17);
            let blob2 = bezier_blob_at(rng, 200.0, 168.0, blob2_r, &p.acc, 0.10);
            let (title_block, metrics) = render_title(
                &lines,
                TitleSpec {
                    x: 36.0,
                    y: 432.0,
                    max_width: 330.0,
                    max_height: 132.0,
                    align: TextAlign::Left,
                    center_block: false,
                    min_size: 20,
                    max_size: 72,
                    single_line_max: 82,
                    letter_spacing: 4.0,
                },
                &p.fg,
            );
            let divider_y = metrics.last_baseline + metrics.font_size as f32 * 0.24;
            let author_y = (divider_y + 42.0).min(574.0);
            let body = format!(
                r#"<rect width="{WIDTH}" height="{HEIGHT}" fill="{bg}"/>{blob1}{blob2}<line x1="36" y1="398" x2="{rule_end}" y2="398" stroke="{muted}" stroke-width="1.6"/>{title}<line x1="36" y1="{divider_y:.1}" x2="364" y2="{divider_y:.1}" stroke="{acc}" stroke-width="1.2" opacity="0.62"/>{author}{grain}"#,
                bg = p.grad_a,
                blob1 = blob1,
                blob2 = blob2,
                muted = p.muted,
                acc = p.acc,
                rule_end = rng.random_range(108..=176),
                title = title_block,
                author = author_text(author, 36.0, author_y, TextAlign::Left, &p.fg, 0.58),
                grain = grain_overlay(0.08)
            );
            svg_document("", &body)
        }
        TemplateKind::CornerOrnaments => {
            let (title_block, metrics) = render_title(
                &lines,
                TitleSpec {
                    x: 200.0,
                    y: 320.0,
                    max_width: 280.0,
                    max_height: 138.0,
                    align: TextAlign::Center,
                    center_block: true,
                    min_size: 20,
                    max_size: 72,
                    single_line_max: 82,
                    letter_spacing: 5.0,
                },
                &p.fg,
            );
            let divider_y = metrics.last_baseline + metrics.font_size as f32 * 0.26;
            let body = format!(
                r#"<rect width="{WIDTH}" height="{HEIGHT}" fill="{bg}"/><path d="M30,30 L30,84 M30,30 L84,30" stroke="{acc}" stroke-width="3" fill="none" opacity="0.72"/><path d="M370,30 L370,84 M370,30 L316,30" stroke="{acc}" stroke-width="3" fill="none" opacity="0.72"/><path d="M30,570 L30,516 M30,570 L84,570" stroke="{acc}" stroke-width="3" fill="none" opacity="0.72"/><path d="M370,570 L370,516 M370,570 L316,570" stroke="{acc}" stroke-width="3" fill="none" opacity="0.72"/>{title}<line x1="110" y1="{divider_y:.1}" x2="290" y2="{divider_y:.1}" stroke="{acc}" stroke-width="1.2" opacity="0.64"/>{author}{grain}"#,
                bg = p.bg,
                acc = p.acc,
                title = title_block,
                author = author_text(
                    author,
                    200.0,
                    divider_y + 38.0,
                    TextAlign::Center,
                    &p.fg,
                    0.56
                ),
                grain = grain_overlay(0.05)
            );
            svg_document("", &body)
        }
        TemplateKind::WaveStack => {
            let (title_block, metrics) = render_title(
                &lines,
                TitleSpec {
                    x: 36.0,
                    y: 430.0,
                    max_width: 330.0,
                    max_height: 112.0,
                    align: TextAlign::Left,
                    center_block: false,
                    min_size: 20,
                    max_size: 72,
                    single_line_max: 82,
                    letter_spacing: 5.0,
                },
                &p.fg,
            );
            let divider_y = metrics.last_baseline + metrics.font_size as f32 * 0.24;
            let mut waves = String::new();
            for (idx, (y, amp)) in [104.0, 152.0, 200.0, 248.0, 296.0]
                .into_iter()
                .zip([20.0, 18.0, 22.0, 18.0, 20.0])
                .enumerate()
            {
                let opacity = 0.28 + idx as f32 * 0.08;
                let stroke = 1.4 + (idx % 2) as f32 * 0.4;
                waves.push_str(&format!(
                    r#"<path d="{}" fill="none" stroke="{}" stroke-width="{stroke:.1}" opacity="{opacity:.2}"/>"#,
                    wave_path(y, amp, WIDTH as f32 + 20.0),
                    p.acc
                ));
            }
            let body = format!(
                r#"<rect width="{WIDTH}" height="{HEIGHT}" fill="{bg}"/>{waves}{title}<line x1="36" y1="{divider_y:.1}" x2="364" y2="{divider_y:.1}" stroke="{acc}" stroke-width="1.1" opacity="0.62"/>{author}{grain}"#,
                bg = p.grad_a,
                waves = waves,
                title = title_block,
                acc = p.acc,
                author = author_text(author, 36.0, 574.0, TextAlign::Left, &p.fg, 0.58),
                grain = grain_overlay(0.04)
            );
            svg_document("", &body)
        }
        TemplateKind::HalftoneDots => {
            let defs = format!(
                r#"<pattern id="dots" width="24" height="24" patternUnits="userSpaceOnUse"><circle cx="12" cy="12" r="3.4" fill="{}" opacity="0.34"/></pattern>"#,
                p.acc
            );
            let (title_block, metrics) = render_title(
                &lines,
                TitleSpec {
                    x: 200.0,
                    y: 378.0,
                    max_width: 300.0,
                    max_height: 132.0,
                    align: TextAlign::Center,
                    center_block: true,
                    min_size: 20,
                    max_size: 72,
                    single_line_max: 82,
                    letter_spacing: 5.0,
                },
                &p.fg,
            );
            let band_y = 280.0 + rng.random_range(-18.0..18.0);
            let band_h = 152.0;
            let divider_y = metrics.last_baseline + metrics.font_size as f32 * 0.26;
            let body = format!(
                r#"<rect width="{WIDTH}" height="{HEIGHT}" fill="{bg}"/><rect width="{WIDTH}" height="{HEIGHT}" fill="url(#dots)"/><rect x="0" y="{band_y:.1}" width="{WIDTH}" height="{band_h:.1}" fill="{bg}" opacity="0.93"/>{title}<line x1="64" y1="{divider_y:.1}" x2="336" y2="{divider_y:.1}" stroke="{acc}" stroke-width="1.2" opacity="0.62"/>{author}{grain}"#,
                bg = p.bg,
                title = title_block,
                acc = p.acc,
                author = author_text(author, 200.0, 574.0, TextAlign::Center, &p.fg, 0.70),
                grain = grain_overlay(0.03)
            );
            svg_document(&defs, &body)
        }
        TemplateKind::OverlappingCircles => {
            let c1 = (136.0, 236.0, 136.0);
            let c2 = (264.0, 192.0, 124.0);
            let c3 = (200.0, 300.0, 108.0);
            let (title_block, metrics) = render_title(
                &lines,
                TitleSpec {
                    x: 36.0,
                    y: 460.0,
                    max_width: 330.0,
                    max_height: 120.0,
                    align: TextAlign::Left,
                    center_block: false,
                    min_size: 18,
                    max_size: 72,
                    single_line_max: 82,
                    letter_spacing: 5.0,
                },
                &p.fg,
            );
            let divider_y = metrics.last_baseline + metrics.font_size as f32 * 0.24;
            let author_y = (divider_y + 40.0).min(572.0);
            let body = format!(
                r#"<rect width="{WIDTH}" height="{HEIGHT}" fill="{bg}"/><circle cx="{x1}" cy="{y1}" r="{r1}" fill="{acc}" opacity="0.12"/><circle cx="{x2}" cy="{y2}" r="{r2}" fill="{acc}" opacity="0.10"/><circle cx="{x3}" cy="{y3}" r="{r3}" fill="{acc}" opacity="0.08"/><circle cx="{x1}" cy="{y1}" r="{r1}" fill="none" stroke="{acc}" stroke-width="1.2" opacity="0.30"/><circle cx="{x2}" cy="{y2}" r="{r2}" fill="none" stroke="{acc}" stroke-width="1.2" opacity="0.24"/>{title}<line x1="36" y1="{divider_y:.1}" x2="364" y2="{divider_y:.1}" stroke="{acc}" stroke-width="1.1" opacity="0.52"/>{author}{grain}"#,
                bg = p.grad_a,
                acc = p.acc,
                x1 = c1.0,
                y1 = c1.1,
                r1 = c1.2,
                x2 = c2.0,
                y2 = c2.1,
                r2 = c2.2,
                x3 = c3.0,
                y3 = c3.1,
                r3 = c3.2,
                title = title_block,
                author = author_text(author, 36.0, author_y, TextAlign::Left, &p.fg, 0.56),
                grain = grain_overlay(0.06)
            );
            svg_document("", &body)
        }
        TemplateKind::StackedChevrons => {
            let (title_block, metrics) = render_title(
                &lines,
                TitleSpec {
                    x: 200.0,
                    y: 460.0,
                    max_width: 300.0,
                    max_height: 118.0,
                    align: TextAlign::Center,
                    center_block: true,
                    min_size: 18,
                    max_size: 72,
                    single_line_max: 82,
                    letter_spacing: 5.0,
                },
                &p.fg,
            );
            let divider_y = metrics.last_baseline + metrics.font_size as f32 * 0.26;
            let mut chevrons = String::new();
            for (idx, y) in [56.0, 106.0, 156.0, 206.0, 256.0].into_iter().enumerate() {
                let opacity = 0.30 - idx as f32 * 0.04;
                chevrons.push_str(&format!(
                    r#"<polyline points="0,{y:.1} 200,{mid:.1} 400,{y:.1}" fill="none" stroke="{acc}" stroke-width="3" opacity="{opacity:.2}"/>"#,
                    mid = y + 54.0,
                    acc = p.acc
                ));
            }
            let body = format!(
                r#"<rect width="{WIDTH}" height="{HEIGHT}" fill="{bg}"/>{chevrons}{title}<line x1="60" y1="{divider_y:.1}" x2="340" y2="{divider_y:.1}" stroke="{acc}" stroke-width="1.2" opacity="0.60"/>{author}{grain}"#,
                bg = p.bg,
                chevrons = chevrons,
                title = title_block,
                acc = p.acc,
                author = author_text(author, 200.0, 574.0, TextAlign::Center, &p.fg, 0.65),
                grain = grain_overlay(0.04)
            );
            svg_document("", &body)
        }
        TemplateKind::LayeredObliques => {
            let (title_block, metrics) = render_title(
                &lines,
                TitleSpec {
                    x: 36.0,
                    y: 470.0,
                    max_width: 330.0,
                    max_height: 126.0,
                    align: TextAlign::Left,
                    center_block: false,
                    min_size: 18,
                    max_size: 72,
                    single_line_max: 82,
                    letter_spacing: 5.0,
                },
                &p.fg,
            );
            let divider_y = metrics.last_baseline + metrics.font_size as f32 * 0.24;
            let author_y = (divider_y + 36.0).min(584.0);
            let y2 = rng.random_range(254.0..308.0);
            let body = format!(
                r#"<rect width="{WIDTH}" height="{HEIGHT}" fill="{bg}"/><polygon points="0,110 344,30 400,110 56,190" fill="{acc}" opacity="0.10"/><polygon points="0,210 352,120 400,200 48,290" fill="{acc}" opacity="0.08"/><polygon points="0,300 360,200 400,286 40,386" fill="{acc}" opacity="0.06"/><polygon points="0,390 364,{y2:.1} 400,364 36,464" fill="{acc}" opacity="0.05"/>{title}<line x1="36" y1="{divider_y:.1}" x2="364" y2="{divider_y:.1}" stroke="{acc}" stroke-width="1.1" opacity="0.50"/>{author}{grain}"#,
                bg = p.grad_a,
                acc = p.acc,
                y2 = y2,
                title = title_block,
                author = author_text(author, 36.0, author_y, TextAlign::Left, &p.fg, 0.54),
                grain = grain_overlay(0.05)
            );
            svg_document("", &body)
        }
    }
}
