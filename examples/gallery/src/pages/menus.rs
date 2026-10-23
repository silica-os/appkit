//! MenusDoc: Try them (the menu), Menu items, Menus.

use appkit_ui::prelude::*;

use super::{code, h2, lead, page, stage};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let last = cx.state(String::new);
        let (a, b, c, d) = (last.clone(), last.clone(), last.clone(), last.clone());
        let (m1, m2, m3, m4) = (last.clone(), last.clone(), last.clone(), last.clone());
        let file = pull_down(
            "File",
            &[
                "New Window",
                "Open Recent",
                "Send a Link",
                "Autosave",
                "Duplicate",
                "Export…",
                "Move to Trash",
            ],
            move |i| {
                let names = [
                    "New Window",
                    "Open Recent",
                    "Send a Link",
                    "Autosave",
                    "Duplicate",
                    "Export…",
                    "Move to Trash",
                ];
                a.set(names.get(i).copied().unwrap_or_default().to_string())
            },
        );
        let area = label("Secondary-click here for the menu.")
            .padding(24.0)
            .background(rgba(0x80808022))
            .corner_radius(12.0)
            .context_menu(vec![
                MenuEntry::item("New Window", move || b.set("New Window".into())).shortcut("cmd+n"),
                MenuEntry::item("Duplicate", move || c.set("Duplicate".into())).shortcut("cmd+d"),
                MenuEntry::separator(),
                MenuEntry::item("Move to Trash", move || d.set("Move to Trash".into())),
            ]);
        page(vec![
            lead(
                "Menus are plain glass blocks of items you can place anywhere; a popover anchors any content (or a menu) to a trigger; a tooltip waits 700 ms on the first hover and shows at once when you move between tooltips.",
            ),
            h2("Try them"),
            stage(
                row((
                    file,
                    area,
                    label(if last.get().is_empty() {
                        "open the File menu, use ↑ ↓ Enter Esc".to_string()
                    } else {
                        format!("chose: {}", last.get())
                    })
                    .color("Text/Secondary"),
                ))
                .spacing(16.0)
                .alignment(4),
            ),
            code("pull_down(\"File\", &[\"New Window\", …], |i| …)"),
            h2("Menu items"),
            lead(
                "Secondary-click the panel: a heading, a checked and a mixed item, an icon, a submenu, the destructive and the disabled look.",
            ),
            label("Secondary-click here for the menu items.")
                .padding(24.0)
                .background(rgba(0x80808022))
                .corner_radius(12.0)
                .context_menu(vec![
                    MenuEntry::heading("Document"),
                    MenuEntry::item("Autosave", move || m1.set("Autosave".into())).checked(true),
                    MenuEntry::item("Show Ruler", move || m2.set("Show Ruler".into())).mixed(),
                    MenuEntry::item("Duplicate", move || m3.set("Duplicate".into()))
                        .icon("copy")
                        .shortcut("cmd+d"),
                    MenuEntry::submenu(
                        "Open Recent",
                        vec![
                            MenuEntry::item("notes.txt", || {}),
                            MenuEntry::item("recipes.md", || {}),
                        ],
                    ),
                    MenuEntry::separator(),
                    MenuEntry::item("Export…", || {}).disabled(),
                    MenuEntry::item("Move to Trash", move || m4.set("Move to Trash".into()))
                        .destructive(),
                ]),
            h2("Menus"),
            super::wrapped(
                "The menus above are the real ones: glass, 12 pt corners, the highlighted row follows ↑ ↓.",
                "Callout",
                "Text/Secondary",
                680.0,
            ),
        ])
    }
}
