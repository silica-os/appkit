//! Collections: a list box, a table, an outline and an icon grid.

use appkit_ui::prelude::*;
use appkit_ui::{Column, collection, outline, symbol, table};

use super::section;

/// The page.
pub struct Page;

/// A file of the table demo.
#[derive(Clone)]
struct File {
    name: String,
    size: u64,
    kind: &'static str,
}

fn files() -> Vec<File> {
    let kinds = ["Document", "Image", "Folder", "Archive", "Text"];
    (0..2000u64)
        .map(|i| File {
            name: format!("File {:04}", i + 1),
            size: (i * 7919) % 100_000,
            kind: kinds.get((i % 5) as usize).copied().unwrap_or("Document"),
        })
        .collect()
}

fn cell_label(text: String, secondary: bool) -> Element {
    let l = label(text).align(3);
    if secondary {
        l.text_style("Callout").color("Text/Secondary")
    } else {
        l
    }
}

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let data = cx.state(files);
        let sort = cx.state(|| None::<(usize, bool)>);
        let picked = cx.state(|| vec![2u64]);
        // The rows in the sorted order.
        let mut order: Vec<usize> = (0..data.get().len()).collect();
        if let Some((col, asc)) = sort.get() {
            let d = data.get();
            order.sort_by(|a, b| {
                let (Some(x), Some(y)) = (d.get(*a), d.get(*b)) else {
                    return std::cmp::Ordering::Equal;
                };
                let o = match col {
                    0 => x.name.cmp(&y.name),
                    1 => x.size.cmp(&y.size),
                    _ => x.kind.cmp(y.kind),
                };
                if asc { o } else { o.reverse() }
            });
        }
        let order = std::rc::Rc::new(order);
        let rows = data.get().len();
        let (d2, o2) = (data.clone(), order.clone());
        let (d3, o3) = (data.clone(), order.clone());
        let s2 = sort.clone();
        let p2 = picked.clone();
        let table_view = table(
            vec![
                Column::new("Name", 260.0).editable(),
                Column::new("Size", 100.0).align(2),
                Column::new("Kind", 120.0),
            ],
            rows,
            move |r, c| {
                let d = d2.get();
                let Some(f) = o2.get(r).and_then(|i| d.get(*i)) else {
                    return label("");
                };
                match c {
                    0 => row((
                        symbol(if f.kind == "Folder" { "folder" } else { "doc" })
                            .color("Accent/Amalfi"),
                        label(f.name.clone()),
                    ))
                    .spacing(4.0)
                    .align(3),
                    1 => cell_label(format!("{} KB", f.size), true).align(5),
                    _ => cell_label(f.kind.to_string(), true),
                }
            },
        )
        .alternating(true)
        .sorted(sort.get())
        .on_sort(move |s| s2.set(s))
        .on_edit(move |r, _, text| {
            let i = o3.get(r as usize).copied();
            d3.update(|d| {
                if let Some(f) = i.and_then(|i| d.get_mut(i)) {
                    f.name = text.clone();
                }
            });
        })
        .selection(&picked.get())
        .on_select(move |keys| p2.set(keys))
        .id("table")
        .fill_width()
        .height(220.0);

        let tree = outline(
            vec![Column::new("Folder", 200.0), Column::new("Items", 60.0)],
            |parent| match parent {
                None => vec![1, 2, 3],
                Some(n) if n < 1000 => (1..=4).map(|k| n * 10 + k).collect(),
                Some(_) => Vec::new(),
            },
            |n| n < 1000,
            |n, c| {
                if c == 0 {
                    row((
                        symbol(if n < 1000 { "folder" } else { "doc" }).color("Accent/Amalfi"),
                        label(format!("Item {n}")),
                    ))
                    .spacing(4.0)
                    .align(3)
                } else {
                    cell_label(if n < 1000 { "4".into() } else { "—".into() }, true)
                }
            },
        )
        .style(2)
        .id("outline")
        .width(380.0)
        .height(220.0);

        let tiles = collection(500, (104.0, 104.0), |i| {
            column((
                symbol(if i % 3 == 0 { "folder" } else { "doc" })
                    .color("Accent/Amalfi")
                    .font_size(48.0)
                    .frame(64.0, 64.0),
                label(format!("Item {}", i + 1))
                    .text_style("Callout")
                    .text_align(1)
                    .frame(88.0, 16.0),
            ))
            .spacing(10.0)
            .align(1)
        })
        .id("grid")
        .fill_width()
        .height(220.0);

        column((
            section(
                "A list box of 10 000 rows",
                appkit_ui::list(10_000, 24.0, |i| {
                    label(format!("Row {}", i + 1))
                        .padding_xy(8.0, 0.0)
                        .align(3)
                })
                .style(2)
                .id("list")
                .fill_width()
                .height(120.0),
            ),
            section(
                &format!(
                    "Table of {} files: click a title to sort, Return renames ({} selected)",
                    rows,
                    picked.get().len()
                ),
                table_view,
            ),
            row((
                section("Outline", tree),
                section("Collection of 500 tiles", tiles),
            ))
            .spacing(24.0)
            .alignment(0)
            .fill_width(),
        ))
        .spacing(18.0)
        .alignment(0)
        .fill()
        .align(0)
    }
}
