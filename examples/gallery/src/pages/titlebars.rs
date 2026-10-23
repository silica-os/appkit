//! WindowsDoc: Window controls, Title bars, Toolbar pieces, Tabs.

use appkit_ui::prelude::*;
use appkit_ui::tabs;

use super::{h2, lead, page, stage};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let q = cx.state(String::new);
        let q2 = cx.state(String::new);
        let view = cx.state(|| 0usize);
        let titles = cx.state(|| vec!["Welcome".to_string(), "Notes".to_string()]);
        let selected = cx.state(|| 0usize);
        let t = titles.get();
        let pages: Vec<Element> = t
            .iter()
            .map(|title| {
                label(title.clone())
                    .text_style("Title 3")
                    .padding(16.0)
                    .fill()
                    .align(0)
                    .key(title.clone())
            })
            .collect();
        let (t2, t3, s2, s3) = (
            titles.clone(),
            titles.clone(),
            selected.clone(),
            selected.clone(),
        );
        let unified = toolbar((
            toolbar_button("sidebar").accessibility_label("Sidebar"),
            toolbar_group(
                &[("chevron-left", "Back"), ("chevron-right", "Forward")],
                |_| {},
            ),
            toolbar_title("Lemon cake"),
            spacer(),
            segmented(&["List", "Grid", "Columns"], &view.binding()),
            toolbar_search("Search", &q.binding()),
        ))
        .fill_width()
        .height(52.0);
        let compact = toolbar((
            toolbar_button("chevron-left").accessibility_label("Back"),
            toolbar_title("Lemon cake"),
            spacer(),
        ))
        .fill_width()
        .height(52.0);
        let pieces = toolbar((
            toolbar_button("plus").accessibility_label("Add"),
            toolbar_button("gear").accessibility_label("Settings"),
            toolbar_group(
                &[("chevron-left", "Back"), ("chevron-right", "Forward")],
                |_| {},
            ),
            toolbar_search("Search", &q2.binding()),
        ))
        .fill_width()
        .height(52.0);
        page(vec![
            lead(
                "SWindow assembles a title bar or toolbar (STitlebar with STitlebarPiece, STitlebarTitle, STrafficLights), a sidebar, content and an inspector. Regions are split by 1 pt lines; only the window has a shadow.",
            ),
            h2("Window controls"),
            super::wrapped(
                "The close, minimise and zoom controls are drawn by the window itself (see the Windows page).",
                "Callout",
                "Text/Secondary",
                680.0,
            ),
            h2("Title bars"),
            label("Unified")
                .text_style("Caption")
                .color("Text/Tertiary"),
            stage(unified),
            label("Compact")
                .text_style("Caption")
                .color("Text/Tertiary"),
            stage(compact),
            h2("Toolbar pieces"),
            super::wrapped(
                "Each piece is a 52 pt slot with 4 pt side padding. Buttons are 36 pt glass; inactive windows grey the glyphs only.",
                "Callout",
                "Text/Secondary",
                680.0,
            ),
            stage(pieces),
            h2("Tabs"),
            tabs(
                &t.iter().map(String::as_str).collect::<Vec<_>>(),
                &selected.binding(),
                pages,
            )
            .on_new_tab(move || {
                let n = t2.get().len() + 1;
                t2.update(|v| v.push(format!("Tab {n}")));
                s2.set(n - 1);
            })
            .on_close_tab(move |i| {
                t3.update(|v| {
                    if v.len() > 1 && i < v.len() {
                        v.remove(i);
                    }
                });
                let len = t3.get().len();
                s3.update(|s| *s = (*s).min(len.saturating_sub(1)));
            })
            .fill_width()
            .height(140.0),
        ])
    }
}
