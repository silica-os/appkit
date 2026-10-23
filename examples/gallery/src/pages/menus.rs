//! MenusDoc: Try them (the menu), Menu items, Menus.

use appkit_ui::prelude::*;

use super::{code, h2, lead, page, stage};

/// The page.
pub struct Page;

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let last = cx.state(String::new);
        let (a, b, c, d) = (last.clone(), last.clone(), last.clone(), last.clone());
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
                .alignment(1),
            ),
            code("pull_down(\"File\", &[\"New Window\", …], |i| …)"),
            h2("Menu items"),
            super::wrapped(
                "Items with a checkmark, a submenu, a header, an icon, the destructive and the disabled look are drawn by the native menu: AppKit's declarative MenuEntry has shortcut and separator only, so they are not shown here.",
                "Callout",
                "Text/Tertiary",
                680.0,
            ),
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
