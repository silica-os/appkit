//! Motion: the springs and fades of `silica_motion`, each playable. The button moves a dot
//! along its track with that animation; Reduce Motion (View menu) turns the springs into
//! cross-fades.

use appkit_ui::Animation;
use appkit_ui::kit::Kind;
use appkit_ui::prelude::*;

/// The page.
pub struct Page;

/// A token: its name, what it is, where it is used, and the animation that plays it.
struct Token {
    name: &'static str,
    values: &'static str,
    used: &'static str,
    animation: Animation,
}

const SPRINGS: &[Token] = &[
    Token {
        name: "spring.snappy",
        values: "response 0.25 s, damping 0.9",
        used: "hover and press, switch thumb",
        animation: Animation::Snappy,
    },
    Token {
        name: "spring.panel",
        values: "response 0.35 s, damping 0.82",
        used: "menus, popovers, sheets",
        animation: Animation::Panel,
    },
    Token {
        name: "spring.window",
        values: "response 0.4 s, damping 0.88",
        used: "window open and close",
        animation: Animation::Window,
    },
    Token {
        name: "spring.morph",
        values: "response 0.5 s, damping 0.7",
        used: "shape morphs",
        animation: Animation::Morph,
    },
    Token {
        name: "spring.bouncy",
        values: "response 0.6 s, damping 0.5",
        used: "attention bounce",
        animation: Animation::Bouncy,
    },
];

const FADES: &[Token] = &[
    Token {
        name: "fade.instant",
        values: "100 ms, (0.2, 0, 0, 1)",
        used: "closing transient surfaces",
        animation: Animation::FadeInstant,
    },
    Token {
        name: "fade.quick",
        values: "150 ms, (0.2, 0, 0, 1)",
        used: "content swaps, hover overlays",
        animation: Animation::FadeQuick,
    },
    Token {
        name: "fade.standard",
        values: "250 ms, (0.4, 0, 0.2, 1)",
        used: "appearance changes",
        animation: Animation::FadeStandard,
    },
    Token {
        name: "fade.slow",
        values: "400 ms, (0.4, 0, 0.2, 1)",
        used: "display changes",
        animation: Animation::FadeSlow,
    },
];

/// One playable row.
struct Sample {
    token: &'static Token,
}

impl Component for Sample {
    fn body(&self, cx: &mut Cx) -> Element {
        let at_end = cx.state(|| false);
        let e2 = at_end.clone();
        let t = self.token;
        let lead = if at_end.get() { 220.0 } else { 0.0 };
        row((
            column((
                label(t.name).text_style("Headline"),
                label(t.values)
                    .text_style("Callout")
                    .color("Text/Secondary"),
                label(t.used).text_style("Caption").color("Text/Tertiary"),
            ))
            .spacing(2.0)
            .alignment(0)
            .width(230.0),
            button("Play")
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
        .alignment(1)
    }
}

impl Component for Page {
    fn body(&self, _cx: &mut Cx) -> Element {
        let list = |tokens: &'static [Token]| {
            column(
                tokens
                    .iter()
                    .map(|t| component(Sample { token: t }).key(t.name))
                    .collect::<Vec<_>>(),
            )
            .spacing(14.0)
            .alignment(0)
        };
        page(vec![
            section("Springs", list(SPRINGS)),
            section("Fades", list(FADES)),
        ])
    }
}

/// A titled block of the page.
fn section(title: &str, content: Element) -> Element {
    column((
        label(title).text_style("Headline").color("Text/Secondary"),
        content,
    ))
    .spacing(8.0)
    .alignment(0)
}

/// The page: its sections, top to bottom.
fn page(sections: Vec<Element>) -> Element {
    column(sections)
        .spacing(20.0)
        .alignment(0)
        .padding(24.0)
        .fill_width()
        .align(0)
}
