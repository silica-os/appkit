//! SidebarDoc: the sidebar playground.

use appkit_ui::prelude::*;

use super::{code, h2, lead, page, stage};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let sel = cx.state(|| 0usize);
        let items = [
            ("clock", "Recents"),
            ("folder", "Documents"),
            ("download", "Downloads"),
            ("photo", "Photos"),
            ("folder", "Music"),
        ];
        let rows: Vec<Element> = items
            .iter()
            .enumerate()
            .map(|(i, (s, t))| {
                let sel = sel.clone();
                sidebar_row(s, t, sel.get() == i, move || sel.set(i)).key(i as i64)
            })
            .collect();
        let open = cx.state(|| true);
        let ejected = cx.state(|| false);
        let e2 = ejected.clone();
        let mut volume = sidebar_row("folder", "Backup", false, || {})
            .children(sidebar_eject(move || e2.set(true)));
        if ejected.get() {
            volume = volume.hidden(true);
        }
        let groups = sidebar(vec![
            sidebar_group(
                "Favourites",
                &open.binding(),
                vec![
                    sidebar_row("clock", "Recents", false, || {}).key(0i64),
                    sidebar_row("folder", "Documents", false, || {})
                        .children(sidebar_badge("12"))
                        .key(1i64),
                ],
            ),
            sidebar_group(
                "Locations",
                &cx.state(|| true).binding(),
                vec![volume.key(2i64)],
            ),
            sidebar_footer(|| {}, || {}),
        ])
        .frame(240.0, 240.0);
        let live = row((
            sidebar(rows).frame(240.0, 240.0),
            label(format!(
                "selected: {}",
                items.get(sel.get()).map_or("", |i| i.1)
            ))
            .color("Text/Secondary"),
        ))
        .spacing(16.0)
        .alignment(1);
        page(vec![
            lead(
                "A 240 pt sidebar: rows with a coloured glyph, groups with a disclosure chevron that fold away, count badges, volumes with an eject button, a footer toolbar. v-model is the selected row; arrow keys move.",
            ),
            h2("Playground"),
            row((stage(live), groups)).spacing(16.0).alignment(0),
            code("sidebar(rows).frame(240.0, 240.0)"),
            h2("Rows"),
            super::wrapped(
                "The rows above are the real ones: hover, selection and the inactive-window look.",
                "Callout",
                "Text/Secondary",
                680.0,
            ),
        ])
    }
}
