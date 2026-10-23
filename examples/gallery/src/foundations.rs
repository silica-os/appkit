//! The Foundations of the storybook (`TokenPage`, `Token*.vue`, `MotionTokens.vue`,
//! `AnimationLab.vue`) and its Home, read from AppKit's own copies of the tokens.

use std::rc::Rc;

use appkit::amalfi::tokens::{self, DARK_ALL, LIGHT_ALL, typography};
use appkit_ui::Animation;
use appkit_ui::PValue;
use appkit_ui::kit::{Kind, Prop};
use appkit_ui::prelude::*;

use crate::pages::{captioned, page, section};

/// A colour of the tokens as a view colour.
fn ui_color(c: appkit::color::Color) -> Color {
    rgba((u32::from(c.r) << 24) | (u32::from(c.g) << 16) | (u32::from(c.b) << 8) | u32::from(c.a))
}

fn hex(c: appkit::color::Color) -> String {
    if c.a == 255 {
        format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
    } else {
        format!("#{:02X}{:02X}{:02X}{:02X}", c.r, c.g, c.b, c.a)
    }
}

fn note(text: &str) -> Element {
    label(text).color("Text/Secondary")
}

fn mono(text: &str) -> Element {
    label(text).text_style("Mono").color("Text/Tertiary")
}

// ---------------------------------------------------------------------------------------------
// Home
// ---------------------------------------------------------------------------------------------

/// Home.vue: what the kit is, where to go.
pub struct Home {
    /// Opens a page by its index in the sidebar order.
    pub go: Rc<dyn Fn(usize)>,
    /// How many Library pages there are.
    pub library: usize,
}

impl Component for Home {
    fn body(&self, _cx: &mut Cx) -> Element {
        let tile = |title: &str, text: &str, to: usize| {
            let go = self.go.clone();
            column((
                button(title).style(1).on_activate(move || go(to)),
                label(text)
                    .text_style("Callout")
                    .color("Text/Secondary")
                    .prop(Prop::WRAPS, PValue::Bool(true))
                    .width(250.0),
            ))
            .spacing(6.0)
            .alignment(0)
            .width(250.0)
        };
        let found = 1 + self.library;
        let tiles = grid(
            3,
            vec![
                tile(
                    "Library",
                    &format!(
                        "{} pages, named and ordered like the Sketch kit: use the live controls, press them, type in them, drag them. Light and dark.",
                        self.library
                    ),
                    1,
                ),
                tile(
                    "Tokens",
                    &format!(
                        "{} colours, {} text styles, glass, spacing, radii, shadows.",
                        LIGHT_ALL.len(),
                        typography::ALL.len()
                    ),
                    found,
                ),
                tile(
                    "Motion",
                    "Spring and fade tokens with their settle times.",
                    found + 4,
                ),
                tile(
                    "Animation Lab",
                    "Play a spring or a fade on a component and copy it as Rust.",
                    found + 5,
                ),
            ],
        )
        .spacing(16.0);
        let steps = column((
            note("1. Open a page under Library: every state of the control is live."),
            note("2. Press, type, drag: hover, press, focus and the transitions behave as in the kit."),
            note("3. In a Rust app: use appkit_ui::prelude::*; the controls reuse each other (an alert is built from buttons)."),
        ))
        .spacing(6.0)
        .alignment(0);
        page(vec![
            label("Silica UI Kit").text_style("Title 1"),
            label("The Amalfi 1.0 component library, built to be used: live controls on the design tokens. Light and dark.")
                .text_style("Body")
                .color("Text/Secondary"),
            tiles,
            section("Using the library", steps),
        ])
    }
}

// ---------------------------------------------------------------------------------------------
// Colors
// ---------------------------------------------------------------------------------------------

/// TokenColors.vue: swatches by group, light or dark, filtered by name.
pub struct Colors;

impl Component for Colors {
    fn body(&self, cx: &mut Cx) -> Element {
        let mode = cx.state(|| 0usize);
        let filter = cx.state(String::new);
        let list = if mode.get() == 0 { LIGHT_ALL } else { DARK_ALL };
        let q = filter.get().to_lowercase();
        let mut groups: Vec<(String, Vec<(String, appkit::color::Color)>)> = Vec::new();
        for (name, c) in list {
            if !q.is_empty() && !name.to_lowercase().contains(&q) {
                continue;
            }
            let (g, rest) = name.split_once('/').unwrap_or((name, ""));
            let item = (rest.to_string(), *c);
            match groups.iter_mut().find(|(n, _)| n == g) {
                Some((_, v)) => v.push(item),
                None => groups.push((g.to_string(), vec![item])),
            }
        }
        let bar = row((
            segmented(&["Light", "Dark"], &mode.binding()),
            search_field("Filter tokens", &filter.binding()).width(220.0),
        ))
        .spacing(12.0);
        let mut sections = vec![
            bar,
            note(
                "Copy on click needs a clipboard service, which the declarative layer does not have: not offered.",
            ),
        ];
        for (g, items) in groups {
            let n = items.len();
            let chips: Vec<Element> = items
                .into_iter()
                .map(|(name, c)| {
                    column((
                        view(Kind::VIEW)
                            .frame(120.0, 40.0)
                            .background(ui_color(c))
                            .corner_radius(9.0),
                        label(&name).text_style("Headline"),
                        mono(&hex(c)),
                    ))
                    .spacing(2.0)
                    .alignment(0)
                    .key(name)
                })
                .collect();
            sections.push(section(&format!("{g} ({n})"), grid(5, chips).spacing(14.0)));
        }
        let dark = mode.get() == 1;
        page(sections).background(rgba(if dark { 0x17181AFF } else { 0xFFFFFFFF }))
    }
}

// ---------------------------------------------------------------------------------------------
// Typography
// ---------------------------------------------------------------------------------------------

/// TokenType.vue: every text style with its numbers, set in an editable sample.
pub struct Typography;

impl Component for Typography {
    fn body(&self, cx: &mut Cx) -> Element {
        let sample = cx.state(|| "The quick brown fox jumps over the lazy dog".to_string());
        let rows: Vec<Element> = typography::ALL
            .iter()
            .filter(|t| t.size <= 40.0)
            .map(|t| {
                row((
                    column((
                        label(t.name).text_style("Headline"),
                        mono(&format!(
                            "{} · {}/{} · {}",
                            t.family,
                            t.size,
                            t.line_height.map_or("auto".to_string(), |l| l.to_string()),
                            t.weight
                        )),
                    ))
                    .spacing(2.0)
                    .alignment(0)
                    .width(220.0),
                    label(sample.get()).text_style(t.name),
                ))
                .spacing(16.0)
                .alignment(10)
                .key(t.name)
            })
            .collect();
        page(vec![
            text_field("Sample", &sample.binding()).width(420.0),
            column(rows).spacing(10.0).alignment(0),
            section(
                "Large styles",
                column(
                    typography::ALL
                        .iter()
                        .filter(|t| t.size > 40.0)
                        .map(|t| {
                            captioned(
                                &format!("{} · {}", t.name, t.size),
                                label("Silica").text_style(t.name),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
                .spacing(8.0)
                .alignment(0),
            ),
        ])
    }
}

// ---------------------------------------------------------------------------------------------
// Glass
// ---------------------------------------------------------------------------------------------

/// The blur, saturation and shadow of a glass tier, from `tokens.json` (`glass.<tier>`).
fn glass_numbers(tier: &str, dark: bool) -> Option<String> {
    let json: serde_json::Value =
        serde_json::from_str(include_str!("../../../crates/appkit/data/tokens.json")).ok()?;
    let g = json
        .get("glass")?
        .get(tier)?
        .get(if dark { "dark" } else { "light" })?;
    let blur = g.get("blurs")?.get(0)?;
    let shadow = g.get("shadows")?.get(0)?;
    Some(format!(
        "blur {} · saturation {} · shadow 0, {}, blur {}",
        blur.get("radius")?,
        blur.get("saturation")?,
        shadow.get("y")?,
        shadow.get("blur")?
    ))
}

/// TokenGlass.vue: the glass materials over a colourful backdrop, with their numbers.
pub struct Glass;

impl Component for Glass {
    fn body(&self, cx: &mut Cx) -> Element {
        let mode = cx.state(|| 0usize);
        let dark = mode.get() == 1;
        let stripes = |tier: i64, name: &str, detail: String| {
            let colors = [0xFFB347FFu32, 0xE5338AFF, 0x0E77E6FF, 0x34A853FF];
            let backdrop = row(colors
                .iter()
                .map(|c| {
                    view(Kind::VIEW)
                        .frame(44.0, 96.0)
                        .background(rgba(*c))
                        .key(*c as i64)
                })
                .collect::<Vec<_>>())
            .spacing(0.0);
            column((
                overlay((
                    backdrop,
                    if tier <= 2 {
                        glass_panel(tier, 16.0, label(name).padding_xy(16.0, 12.0))
                    } else {
                        label(name).padding_xy(16.0, 12.0)
                    },
                ))
                .corner_radius(16.0)
                .clipped(),
                mono(&detail),
            ))
            .spacing(6.0)
            .alignment(0)
            .key(name.to_string())
        };
        let mut cards = Vec::new();
        for (i, (name, light, dk)) in [
            ("Glass/Thin", None, None),
            (
                "Glass/Regular",
                Some(&tokens::materials::GLASS_REGULAR_LIGHT),
                Some(&tokens::materials::GLASS_REGULAR_DARK),
            ),
            ("Glass/Thick", None, None),
        ]
        .into_iter()
        .enumerate()
        {
            let _ = (light, dk);
            let detail = glass_numbers(name.trim_start_matches("Glass/"), dark)
                .unwrap_or_else(|| "from tokens.json".to_string());
            cards.push(stripes(i as i64, name, detail));
        }
        let m = if dark {
            tokens::materials::GLASS_CLEAR_DARK
        } else {
            tokens::materials::GLASS_CLEAR_LIGHT
        };
        cards.push(stripes(
            3,
            "Glass/Clear",
            format!("the lens: blur {} · distortion {}", m.blur, m.distortion),
        ));
        page(vec![
            segmented(&["Light", "Dark"], &mode.binding()),
            note("The glass materials over a colourful backdrop: tint, rim, shadow and background blur."),
            grid(2, cards).spacing(16.0),
        ])
        .dark(Some(dark))
    }
}

// ---------------------------------------------------------------------------------------------
// Spacing, radii, shadows
// ---------------------------------------------------------------------------------------------

/// TokenLayout.vue: spacing, control heights, radii and shadows.
pub struct Layout;

impl Component for Layout {
    fn body(&self, _cx: &mut Cx) -> Element {
        use tokens::{control_heights as ch, radii, spacing};
        let bar = |name: &str, v: f32| {
            column((
                view(Kind::VIEW)
                    .frame(v * 3.0, 12.0)
                    .background(rgba(0x0E77E6FF))
                    .corner_radius(2.0),
                label(name).text_style("Headline"),
                mono(&format!("{v} pt")),
            ))
            .spacing(4.0)
            .alignment(0)
            .key(name.to_string())
        };
        let spacings: Vec<Element> = [
            ("xs", spacing::XS),
            ("s", spacing::S),
            ("m", spacing::M),
            ("l", spacing::L),
            ("xl", spacing::XL),
            ("xxl", spacing::XXL),
        ]
        .iter()
        .map(|(n, v)| bar(n, *v))
        .collect();
        let heights: Vec<Element> = [
            ("mini", ch::MINI),
            ("small", ch::SMALL),
            ("regular", ch::REGULAR),
            ("large", ch::LARGE),
            ("toolbar", ch::TOOLBAR),
        ]
        .iter()
        .map(|(n, v)| {
            column((
                view(Kind::VIEW)
                    .frame(96.0, *v)
                    .background(rgba(0x0E77E633))
                    .corner_radius(8.0),
                label(*n).text_style("Headline"),
                mono(&format!("{v} pt")),
            ))
            .spacing(4.0)
            .alignment(0)
            .key(n.to_string())
        })
        .collect();
        let radii_list: Vec<Element> = [
            ("small control", radii::SMALL_CONTROL),
            ("regular control", radii::REGULAR_CONTROL),
            ("field", radii::FIELD),
            ("menu", radii::MENU),
            ("popover", radii::POPOVER),
            ("window", radii::WINDOW),
            ("sheet, alert", radii::SHEET_ALERT),
            ("Faro, launcher", radii::FARO_AND_LAUNCHER),
        ]
        .iter()
        .map(|(n, v)| {
            column((
                view(Kind::VIEW)
                    .frame(72.0, 72.0)
                    .background(rgba(0x0E77E633))
                    .corner_radius(*v),
                label(*n).text_style("Headline"),
                mono(&format!("{v} pt")),
            ))
            .spacing(4.0)
            .alignment(0)
            .key(n.to_string())
        })
        .collect();
        let shadows: Vec<Element> = tokens::shadows::LIGHT
            .iter()
            .enumerate()
            .map(|(i, s)| {
                column((
                    glass_panel(1, 12.0, view(Kind::VIEW).frame(96.0, 64.0)).padding(8.0),
                    label(format!("Shadow/{}", i + 1)).text_style("Headline"),
                    mono(&format!("y {} · blur {}", s.y, s.blur)),
                ))
                .spacing(4.0)
                .alignment(0)
                .key(i as i64)
            })
            .collect();
        let flow = |items: Vec<Element>| grid(6, items).spacing(20.0);
        page(vec![
            note("Every measure sits on a 4 pt grid."),
            section("Spacing (4 pt grid)", flow(spacings)),
            section("Control heights", flow(heights)),
            section("Radii (smooth corners, smoothing 0.6)", flow(radii_list)),
            section("Shadows", flow(shadows)),
        ])
    }
}

// ---------------------------------------------------------------------------------------------
// Motion
// ---------------------------------------------------------------------------------------------

/// A motion token: its name, the numbers, its use, and the animation that plays it.
struct Token {
    name: &'static str,
    line: String,
    used: &'static str,
    animation: Animation,
    /// Progress 0 → 1 sampled over the token's duration (the curve graph).
    curve: Vec<f32>,
}

/// The curve graph: the progress of a token over its duration as columns, the target at the
/// top (a spring's overshoot rises above it).
fn curve_graph(samples: &[f32]) -> Element {
    let top = 56.0f32;
    let cols: Vec<Element> = samples
        .iter()
        .enumerate()
        .map(|(i, v)| {
            view(Kind::VIEW)
                .frame(4.0, (v.clamp(0.0, 1.3) * top).max(1.0))
                .background(rgba(0x0E77E6FF))
                .corner_radius(1.0)
                .key(i as i64)
        })
        .collect();
    row(cols)
        .spacing(1.0)
        .alignment(6)
        .height(top * 1.3)
        .padding_xy(6.0, 4.0)
        .background(rgba(0x80808014))
        .corner_radius(8.0)
}

/// A spring's progress (0 → 1, released from rest) after `t` seconds.
fn spring_progress(response: f32, zeta: f32, t: f32) -> f32 {
    let w = std::f32::consts::TAU / response.max(0.001);
    if zeta < 1.0 {
        let wd = w * (1.0 - zeta * zeta).sqrt();
        1.0 - (-zeta * w * t).exp() * ((wd * t).cos() + (zeta * w / wd) * (wd * t).sin())
    } else {
        1.0 - (-w * t).exp() * (1.0 + w * t)
    }
}

/// Time for a spring released from rest to stay within 0.5 % of its target, and its overshoot.
fn spring_metrics(response: f32, zeta: f32) -> (u32, f32) {
    let (mut settle, mut peak) = (0u32, 1.0f32);
    for ms in (0..4000).step_by(2) {
        let p = spring_progress(response, zeta, ms as f32 / 1000.0);
        peak = peak.max(p);
        if (p - 1.0).abs() > 0.005 {
            settle = ms + 2;
        }
    }
    (settle, (peak - 1.0) * 100.0)
}

fn tokens_list() -> Vec<Token> {
    use tokens::motion as m;
    let spring = |name: &'static str, s: tokens::SpringToken, used, animation| {
        let (ms, over) = spring_metrics(s.response, s.damping_ratio);
        let curve = (0..48)
            .map(|i| {
                spring_progress(
                    s.response,
                    s.damping_ratio,
                    ms as f32 / 1000.0 * i as f32 / 47.0,
                )
            })
            .collect();
        Token {
            name,
            line: format!(
                "response {} s · ζ {} · settles in {ms} ms · overshoot {over:.1} %",
                s.response, s.damping_ratio
            ),
            used,
            animation,
            curve,
        }
    };
    let fade = |name: &'static str, secs: f32, c: tokens::Cubic, used, animation| Token {
        name,
        line: format!(
            "{} ms · cubic-bezier({}, {}, {}, {})",
            (secs * 1000.0).round(),
            c.x1,
            c.y1,
            c.x2,
            c.y2
        ),
        used,
        animation,
        curve: (0..48).map(|i| c.transform(i as f32 / 47.0)).collect(),
    };
    vec![
        spring(
            "spring.snappy",
            m::SPRING_SNAPPY,
            "hover and press, lens grow, switch thumb",
            Animation::Snappy,
        ),
        spring(
            "spring.panel",
            m::SPRING_PANEL,
            "menus and popovers height, sheets",
            Animation::Panel,
        ),
        spring(
            "spring.window",
            m::SPRING_WINDOW,
            "window open and close",
            Animation::Window,
        ),
        spring(
            "spring.morph",
            m::SPRING_MORPH,
            "shape morphs",
            Animation::Morph,
        ),
        spring(
            "spring.bouncy",
            m::SPRING_BOUNCY,
            "attention bounce",
            Animation::Bouncy,
        ),
        fade(
            "fade.instant",
            m::FADE_INSTANT,
            m::FADE_INSTANT_CURVE,
            "closing transient surfaces",
            Animation::FadeInstant,
        ),
        fade(
            "fade.quick",
            m::FADE_QUICK,
            m::FADE_QUICK_CURVE,
            "content swaps, hover overlays",
            Animation::FadeQuick,
        ),
        fade(
            "fade.standard",
            m::FADE_STANDARD,
            m::FADE_STANDARD_CURVE,
            "appearance changes",
            Animation::FadeStandard,
        ),
        fade(
            "fade.slow",
            m::FADE_SLOW,
            m::FADE_SLOW_CURVE,
            "display changes",
            Animation::FadeSlow,
        ),
    ]
}

/// One playable row: the dot travels along its track with the token's animation.
struct Sample {
    token: Rc<Token>,
}

impl Component for Sample {
    fn body(&self, cx: &mut Cx) -> Element {
        let at_end = cx.state(|| false);
        let e2 = at_end.clone();
        let t = &self.token;
        let lead = if at_end.get() { 220.0 } else { 0.0 };
        row((
            column((
                label(t.name).text_style("Headline"),
                label(t.line.as_str())
                    .text_style("Callout")
                    .color("Text/Secondary"),
                label(t.used).text_style("Caption").color("Text/Tertiary"),
            ))
            .spacing(2.0)
            .alignment(0)
            .width(330.0),
            curve_graph(&t.curve),
            button("Replay")
                .style(1)
                .id(t.name)
                .on_activate(move || e2.set(!e2.get())),
            row((
                view(Kind::VIEW).width(lead),
                symbol("circle-fill")
                    .color("Accent/Amalfi")
                    .frame(24.0, 24.0)
                    .animation(t.animation),
            ))
            .spacing(0.0)
            .width(260.0),
        ))
        .spacing(16.0)
        .alignment(4)
    }
}

/// MotionTokens.vue: the springs and fades, each playable.
pub struct Motion;

impl Component for Motion {
    fn body(&self, _cx: &mut Cx) -> Element {
        let rows: Vec<Element> = tokens_list()
            .into_iter()
            .map(|t| {
                let name = t.name;
                component(Sample { token: Rc::new(t) }).key(name)
            })
            .collect();
        page(vec![
            note("Spring and fade tokens. Press Replay to see them move."),
            column(rows).spacing(14.0).alignment(0),
            note("With Reduce Motion on, springs become cross-fades of 150 ms and nothing scales."),
        ])
    }
}

// ---------------------------------------------------------------------------------------------
// Animation Lab
// ---------------------------------------------------------------------------------------------

/// AnimationLab.vue: play a token on a component's tracks (position, scale, opacity) and copy
/// it as Rust. AppKit animates by token only, so the custom spring and curve editors of the
/// storybook are not here.
pub struct Lab;

impl Component for Lab {
    fn body(&self, cx: &mut Cx) -> Element {
        let list = tokens_list();
        let pick = cx.state(|| 0usize);
        let flipped = cx.state(|| false);
        let (mv, sc, op) = (cx.state(|| false), cx.state(|| true), cx.state(|| true));
        let names: Vec<&str> = list.iter().map(|t| t.name).collect();
        let Some(t) = list.get(pick.get()) else {
            return page(vec![note("No tokens.")]);
        };
        let on = flipped.get();
        let f2 = flipped.clone();
        let mut target = view(Kind::VIEW)
            .frame(96.0, 64.0)
            .background(rgba(0x0E77E6FF))
            .corner_radius(16.0)
            .animation(t.animation);
        if op.get() {
            target = target.opacity(if on { 1.0 } else { 0.2 });
        }
        if sc.get() {
            target = target.frame(if on { 96.0 } else { 70.0 }, if on { 64.0 } else { 46.0 });
        }
        let lead = if mv.get() && on { 240.0 } else { 0.0 };
        let stage = row((view(Kind::VIEW).width(lead), target))
            .spacing(0.0)
            .width(420.0)
            .height(100.0)
            .alignment(4);
        let rust = format!(
            "view.animation(Animation::{});",
            t.name
                .split('.')
                .map(|w| {
                    let mut c = w.chars();
                    c.next()
                        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                        .unwrap_or_default()
                })
                .collect::<String>()
        );
        page(vec![
            note(
                "Move a whole view (position, scale, opacity) with a spring or a fade, then copy it as Rust.",
            ),
            row((
                label("Preset"),
                pop_up(&names, &pick.binding()),
                checkbox("Move", &mv.binding()),
                checkbox("Scale", &sc.binding()),
                checkbox("Opacity", &op.binding()),
                button("Play")
                    .style(1)
                    .on_activate(move || f2.set(!f2.get())),
            ))
            .spacing(12.0)
            .alignment(4),
            section("Stage", stage),
            section("Curve", curve_graph(&t.curve)),
            section("Rust", mono(&rust)),
            label(t.line.as_str())
                .text_style("Callout")
                .color("Text/Secondary"),
        ])
    }
}
