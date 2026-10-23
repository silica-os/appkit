use std::cell::Cell;
use std::rc::Rc;

use appkit_rs::{Headless, Prop};

use crate::prelude::*;
use crate::{mount, preview};

struct Counter {
    renders: Rc<Cell<u32>>,
}

impl Component for Counter {
    fn body(&self, cx: &mut Cx) -> Element {
        self.renders.set(self.renders.get() + 1);
        let count = cx.state(|| 0i64);
        let c2 = count.clone();
        column((
            label(format!("Count: {}", count.get())).id("label"),
            button("Increment")
                .id("inc")
                .on_activate(move || c2.update(|c| *c += 1)),
        ))
        .spacing(12.0)
        .padding(20.0)
    }
}

fn host() -> appkit_rs::View {
    let w = appkit_rs::Window::new(300.0, 200.0).unwrap();
    let h = appkit_rs::View::new(appkit_rs::Kind::VIEW).unwrap();
    w.set_content(&h).unwrap();
    w.show().unwrap();
    h
}

#[test]
fn state_changes_re_render_and_update_in_place() {
    let rt = Headless::new().unwrap();
    let renders = Rc::new(Cell::new(0));
    let r2 = renders.clone();
    let root = mount(host(), move || Counter {
        renders: r2.clone(),
    });
    let label = root.host().find("label").unwrap();
    let inc = root.host().find("inc").unwrap();
    assert_eq!(label.get_str(Prop::TEXT).unwrap(), "Count: 0");
    inc.click().unwrap();
    inc.click().unwrap();
    // The two changes are batched into one render on the next turn.
    rt.advance(0.01);
    assert_eq!(renders.get(), 2);
    assert_eq!(label.get_str(Prop::TEXT).unwrap(), "Count: 2");
    // The same views were updated, not replaced.
    assert_eq!(root.host().find("label").unwrap(), label);
    assert_eq!(root.host().find("inc").unwrap(), inc);
}

struct List {
    items: crate::Model<Vec<&'static str>>,
}

impl Component for List {
    fn body(&self, cx: &mut Cx) -> Element {
        cx.watch(&self.items);
        let items = self.items.get();
        column(for_each(items, |s| *s, |s| label(*s).id(s)))
    }
}

#[test]
fn keyed_children_keep_their_views() {
    let rt = Headless::new().unwrap();
    let model = crate::Model::new(vec!["a", "b", "c"]);
    let m2 = model.clone();
    let root = mount(host(), move || List { items: m2.clone() });
    let col = root.host().children().remove(0);
    let b = root.host().find("b").unwrap();
    model.update(|v| v.reverse());
    rt.advance(0.01);
    let order: Vec<String> = col
        .children()
        .iter()
        .map(|v| v.get_str(Prop::TEXT).unwrap())
        .collect();
    assert_eq!(order, ["c", "b", "a"]);
    assert_eq!(root.host().find("b").unwrap(), b);
    model.update(|v| v.retain(|s| *s != "b"));
    rt.advance(0.01);
    assert!(root.host().find("b").is_none());
    assert_eq!(col.children().len(), 2);
}

struct Toggle;

impl Component for Toggle {
    fn body(&self, cx: &mut Cx) -> Element {
        let on = cx.state(|| false);
        let o2 = on.clone();
        let content = if on.get() {
            label("On").id("x")
        } else {
            button("Off").id("x")
        };
        column((
            content,
            button("Flip")
                .id("flip")
                .on_activate(move || o2.update(|v| *v = !*v)),
        ))
    }
}

#[test]
fn kind_changes_replace_the_view() {
    let rt = Headless::new().unwrap();
    let root = mount(host(), || Toggle);
    let before = root.host().find("x").unwrap();
    root.host().find("flip").unwrap().click().unwrap();
    rt.advance(0.01);
    let after = root.host().find("x").unwrap();
    assert_ne!(before, after);
    assert_eq!(after.get_str(Prop::TEXT).unwrap(), "On");
}

#[test]
fn preview_renders_a_png() {
    let _rt = Headless::new().unwrap();
    let renders = Rc::new(Cell::new(0));
    let png = preview((240.0, 120.0), 2.0, false, move || Counter {
        renders: renders.clone(),
    })
    .unwrap();
    assert_eq!(png.get(1..4), Some(&b"PNG"[..]));
}

struct WithSheet {
    shown: Rc<Cell<u32>>,
}

impl Component for WithSheet {
    fn body(&self, cx: &mut Cx) -> Element {
        let open = cx.state(|| false);
        let o2 = open.clone();
        let shown = self.shown.clone();
        column(button("Open").id("open").on_activate(move || o2.set(true)))
            .sheet(&open.binding(), (200.0, 100.0), move || {
                shown.set(shown.get() + 1);
                label("In the sheet")
            })
            .context_menu(vec![
                crate::MenuEntry::item("Copy", || {}),
                crate::MenuEntry::separator(),
            ])
    }
}

#[test]
fn sheets_open_from_state() {
    let rt = Headless::new().unwrap();
    let shown = Rc::new(Cell::new(0));
    let s2 = shown.clone();
    let root = mount(host(), move || WithSheet { shown: s2.clone() });
    assert_eq!(shown.get(), 0);
    root.host().find("open").unwrap().click().unwrap();
    rt.advance(0.01);
    assert_eq!(shown.get(), 1, "the sheet's content rendered once");
}

struct Form;

impl Component for Form {
    fn body(&self, cx: &mut Cx) -> Element {
        let on = cx.state(|| false);
        let level = cx.state(|| 0.5f64);
        let name = cx.state(String::new);
        let seg = cx.state(|| 1usize);
        column((
            checkbox("Enabled", &on.binding()).id("check"),
            switch(&on.binding()).id("switch"),
            slider(&level.binding(), 0.0, 1.0).id("slider"),
            text_field("Name", &name.binding()).id("name"),
            segmented(&["One", "Two", "Three"], &seg.binding()).id("seg"),
            progress(Some(level.get())),
            spinner(),
            label(format!(
                "{} {:.1} {} {}",
                on.get(),
                level.get(),
                name.get(),
                seg.get()
            ))
            .id("summary"),
        ))
        .spacing(8.0)
    }
}

#[test]
fn controls_bind_state() {
    let rt = Headless::new().unwrap();
    let root = mount(host(), || Form);
    let summary = root.host().find("summary").unwrap();
    assert_eq!(summary.get_str(Prop::TEXT).unwrap(), "false 0.5  1");
    // Clicking the checkbox writes the binding; the switch follows on the next render.
    root.host().find("check").unwrap().click().unwrap();
    rt.advance(0.01);
    assert_eq!(summary.get_str(Prop::TEXT).unwrap(), "true 0.5  1");
    assert!(
        root.host()
            .find("switch")
            .unwrap()
            .get_bool(Prop::VALUE)
            .unwrap()
    );
    assert_eq!(
        root.host()
            .find("seg")
            .unwrap()
            .get_int(Prop::SELECTED_INDEX)
            .unwrap(),
        1
    );
}

struct Big {
    prefix: crate::Model<&'static str>,
}

impl Component for Big {
    fn body(&self, cx: &mut Cx) -> Element {
        cx.watch(&self.prefix);
        let p = self.prefix.get();
        crate::list(100_000, 28.0, move |i| {
            label(format!("{p} {i}")).id(&format!("r{i}"))
        })
        .id("list")
        .fill()
    }
}

#[test]
fn lists_make_only_the_rows_in_sight() {
    let rt = Headless::new().unwrap();
    let prefix = crate::Model::new("Row");
    let p2 = prefix.clone();
    let w = appkit_rs::Window::new(300.0, 200.0).unwrap();
    let h = appkit_rs::View::new(appkit_rs::Kind::VIEW).unwrap();
    w.set_content(&h).unwrap();
    w.show().unwrap();
    let root = mount(h, move || Big { prefix: p2.clone() });
    // A frame lays the list out.
    w.snapshot_png(1.0).unwrap();
    let list = root.host().find("list").unwrap();
    // The rows live in the list's body.
    let live = list.children()[0].children().len();
    assert!(live > 4 && live < 20, "{live} rows");
    let r0 = root.host().find("r0").unwrap();
    assert_eq!(r0.get_str(Prop::TEXT).unwrap(), "Row 0");
    // A render updates the live rows in place.
    prefix.update(|p| *p = "Item");
    rt.advance(0.01);
    w.snapshot_png(1.0).unwrap();
    assert_eq!(r0.get_str(Prop::TEXT).unwrap(), "Item 0");
    assert_eq!(root.host().find("r0").unwrap(), r0);
}

struct Collections {
    sorted: Rc<Cell<i64>>,
}

impl Component for Collections {
    fn body(&self, _cx: &mut Cx) -> Element {
        let sorted = self.sorted.clone();
        column((
            crate::table(
                vec![
                    crate::Column::new("Name", 120.0),
                    crate::Column::new("Size", 60.0),
                ],
                1000,
                |r, c| label(format!("{r}:{c}")).id(&format!("t{r}.{c}")),
            )
            .on_sort(move |s| sorted.set(s.map_or(-99, |(c, a)| if a { c as i64 } else { -1 })))
            .id("table")
            .height(150.0)
            .fill_width(),
            crate::outline(
                vec![crate::Column::new("Name", 120.0)],
                |p| match p {
                    None => vec![1, 2],
                    Some(n) if n < 10 => vec![n * 10, n * 10 + 1],
                    Some(_) => vec![],
                },
                |n| n < 10,
                |n, _| label(format!("node {n}")).id(&format!("o{n}")),
            )
            .id("outline")
            .height(120.0)
            .fill_width(),
            crate::collection(100, (104.0, 104.0), |i| {
                label(format!("tile {i}")).id(&format!("g{i}"))
            })
            .id("grid")
            .height(240.0)
            .fill_width(),
        ))
        .fill()
    }
}

#[test]
fn tables_outlines_and_collections_build_their_items() {
    let _rt = Headless::new().unwrap();
    let sorted = Rc::new(Cell::new(7));
    let s2 = sorted.clone();
    let w = appkit_rs::Window::new(500.0, 600.0).unwrap();
    let h = appkit_rs::View::new(appkit_rs::Kind::VIEW).unwrap();
    h.set_float(Prop::MAX_WIDTH, f64::INFINITY).unwrap();
    h.set_float(Prop::MAX_HEIGHT, f64::INFINITY).unwrap();
    w.set_content(&h).unwrap();
    w.show().unwrap();
    let root = mount(h, move || Collections { sorted: s2.clone() });
    w.snapshot_png(1.0).unwrap();
    let host = root.host();
    assert_eq!(
        host.find("t3.1").unwrap().get_str(Prop::TEXT).unwrap(),
        "3:1"
    );
    assert!(
        host.find("t900.0").is_none(),
        "rows out of sight are not made"
    );
    let table = host.find("table").unwrap();
    let cols = table.get_str(Prop::COLUMN_TITLES).unwrap();
    assert_eq!(cols, "Name\nSize");
    assert!(host.find("o1").is_some() && host.find("o2").is_some());
    assert!(host.find("o10").is_none(), "collapsed");
    let outline = host.find("outline").unwrap();
    outline.set_ints(Prop::EXPANDED_ITEMS, &[1]).unwrap();
    w.snapshot_png(1.0).unwrap();
    assert!(host.find("o10").is_some(), "expanded");
    assert_eq!(outline.get_ints(Prop::EXPANDED_ITEMS).unwrap(), vec![1]);
    assert!(host.find("g0").is_some());
    let grid = host.find("grid").unwrap();
    assert_eq!(grid.get_int(Prop::COLUMNS).unwrap(), 4);
    assert_eq!(sorted.get(), 7);
}

struct Outer {
    flag: crate::Model<bool>,
}

impl Component for Outer {
    fn body(&self, cx: &mut Cx) -> Element {
        cx.watch(&self.flag);
        let f = self.flag.get();
        column((
            label(if f { "on" } else { "off" }),
            crate::component(Collections {
                sorted: Rc::new(Cell::new(0)),
            }),
        ))
        .fill()
    }
}

#[test]
fn collections_keep_their_state_across_renders() {
    let rt = Headless::new().unwrap();
    let flag = crate::Model::new(false);
    let f2 = flag.clone();
    let w = appkit_rs::Window::new(500.0, 700.0).unwrap();
    let h = appkit_rs::View::new(appkit_rs::Kind::VIEW).unwrap();
    h.set_float(Prop::MAX_WIDTH, f64::INFINITY).unwrap();
    h.set_float(Prop::MAX_HEIGHT, f64::INFINITY).unwrap();
    w.set_content(&h).unwrap();
    w.show().unwrap();
    let root = mount(h, move || Outer { flag: f2.clone() });
    w.snapshot_png(1.0).unwrap();
    let host = root.host();
    let outline = host.find("outline").unwrap();
    outline.set_ints(Prop::EXPANDED_ITEMS, &[1]).unwrap();
    let grid = host.find("grid").unwrap();
    grid.set_point(Prop::SCROLL_OFFSET, 0.0, 120.0).unwrap();
    w.snapshot_png(1.0).unwrap();
    assert!(host.find("o10").is_some());
    flag.update(|f| *f = true);
    rt.advance(0.01);
    w.snapshot_png(1.0).unwrap();
    assert_eq!(host.find("outline").unwrap(), outline, "the same view");
    assert!(host.find("o10").is_some(), "still expanded");
    assert_eq!(outline.get_ints(Prop::EXPANDED_ITEMS).unwrap(), vec![1]);
    let g = host.find("grid").unwrap();
    assert_eq!(g, grid);
}

struct Settings;

impl Component for Settings {
    fn body(&self, cx: &mut Cx) -> Element {
        let on = cx.state(|| true);
        let name = cx.state(|| "Studio".to_string());
        form(
            Some("Sharing"),
            (
                form_section(
                    Some("Computer"),
                    (
                        form_row_with(
                            "Name:",
                            text_field("Name", &name.binding()).id("name"),
                            Some(("Other computers see this name.", false)),
                            true,
                        )
                        .id("row"),
                        form_row("Visible:", switch(&on.binding())),
                    ),
                ),
                form_footer((button("Cancel"), button("Done").style(0))),
            ),
        )
        .id("form")
        .toolbar(
            toolbar((
                toolbar_title("Settings").id("title"),
                spacer(),
                toolbar_button("gear").id("gear"),
            ))
            .id("bar"),
        )
    }
}

#[test]
fn forms_and_toolbars_build_the_kit_pieces() {
    let _rt = Headless::new().unwrap();
    let w = appkit_rs::Window::new(600.0, 500.0).unwrap();
    let h = appkit_rs::View::new(appkit_rs::Kind::VIEW).unwrap();
    h.set_float(Prop::MAX_WIDTH, f64::INFINITY).unwrap();
    h.set_float(Prop::MAX_HEIGHT, f64::INFINITY).unwrap();
    w.set_content(&h).unwrap();
    w.show().unwrap();
    let root = mount(h, || Settings);
    w.snapshot_png(1.0).unwrap();
    let host = root.host();
    let bar = host.find("bar").unwrap().frame().unwrap();
    assert_eq!(bar.height, 52.0);
    let gear = host.find("gear").unwrap().frame().unwrap();
    assert_eq!(bar.x + bar.width - (gear.x + gear.width), 16.0);
    let title = host.find("title").unwrap().frame().unwrap();
    assert_eq!(title.x - bar.x, 100.0);
    let field = host.find("name").unwrap().frame().unwrap();
    let row = host.find("row").unwrap().frame().unwrap();
    // The control starts after the 112 pt label column and the 12 pt gap, and fills 204.
    assert_eq!(field.x - row.x, 124.0);
    assert_eq!(field.width, 204.0);
    assert!(host.find("form").unwrap().frame().unwrap().y >= bar.y + 52.0);
}

struct Split;

impl Component for Split {
    fn body(&self, _cx: &mut Cx) -> Element {
        let rows: Vec<Element> = (0..30)
            .map(|i| sidebar_row("square", "Row", false, || {}).key(i as i64))
            .collect();
        row((
            column((scroll_view(sidebar(rows).width(100.0).id("panel"))
                .id("scroll")
                .fill(),))
            .width(100.0)
            .max_height(f32::INFINITY),
            label("Content").fill(),
        ))
        .spacing(0.0)
        .alignment(0)
        .fill()
    }
}

#[test]
fn the_sidebar_scrolls_when_it_is_taller_than_the_window() {
    let rt = Headless::new().unwrap();
    let w = appkit_rs::Window::new(300.0, 200.0).unwrap();
    let h = appkit_rs::View::new(appkit_rs::Kind::VIEW).unwrap();
    h.set_float(Prop::MAX_WIDTH, f64::INFINITY).unwrap();
    h.set_float(Prop::MAX_HEIGHT, f64::INFINITY).unwrap();
    w.set_content(&h).unwrap();
    w.show().unwrap();
    let root = mount(h, || Split);
    rt.advance(0.05);
    let _ = w.snapshot_png(1.0).unwrap();
    let scroll = root.host().find("scroll").unwrap();
    assert!(
        scroll.frame().unwrap().height <= 200.0,
        "{:?}",
        scroll.frame()
    );
    // The rows are not squeezed into the window: there is something to scroll.
    let panel = root.host().find("panel").unwrap();
    assert!(panel.frame().unwrap().height > 200.0, "{:?}", panel.frame());
}

struct WithAlert {
    suppressed: Rc<Cell<Option<bool>>>,
}

impl Component for WithAlert {
    fn body(&self, cx: &mut Cx) -> Element {
        let open = cx.state(|| false);
        let o2 = open.clone();
        let s = self.suppressed.clone();
        column(button("Open").id("open").on_activate(move || o2.set(true))).alert(
            &open.binding(),
            crate::AlertSpec::new("Delete?", &["Cancel", "Delete"], |_| {})
                .message("Gone for good.")
                .glass()
                .critical()
                .stacked()
                .suppression("Do not show this again", move |on| s.set(Some(on)))
                .field("Reason", |_| {}),
        )
    }
}

#[test]
fn an_alert_with_a_suppression_checkbox_and_a_field_opens() {
    let rt = Headless::new().unwrap();
    let suppressed = Rc::new(Cell::new(None));
    let s2 = suppressed.clone();
    let root = mount(host(), move || WithAlert {
        suppressed: s2.clone(),
    });
    root.host().find("open").unwrap().click().unwrap();
    rt.advance(0.01);
    // Nothing was ticked: the callback has not run, and presenting did not fail.
    assert_eq!(suppressed.get(), None);
    let spec = crate::AlertSpec::new("T", &["OK"], |_| {})
        .glass()
        .suppression("x", |_| {})
        .field("y", |_| {});
    assert!(spec.glass && spec.suppression.is_some() && spec.field.is_some());
}

struct SideProbe {
    seen: Rc<Cell<(u32, u32)>>,
}

impl Component for SideProbe {
    fn body(&self, cx: &mut Cx) -> Element {
        let open = cx.state(|| false);
        let e = button("x")
            .popover(&open.binding(), (10.0, 10.0), || label("p"))
            .popover_side(2);
        let after = e.effects.popover.as_ref().map_or(99, |p| p.side);
        let e = e.popover_side(9);
        let below = e.effects.popover.as_ref().map_or(99, |p| p.side);
        self.seen.set((after, below));
        e
    }
}

#[test]
fn a_popover_keeps_its_preferred_side() {
    let _rt = Headless::new().unwrap();
    let seen = Rc::new(Cell::new((0, 0)));
    let s2 = seen.clone();
    let _root = mount(host(), move || SideProbe { seen: s2.clone() });
    assert_eq!(seen.get(), (appkit_rs::side::AFTER, appkit_rs::side::BELOW));
}

struct Segs;

impl Component for Segs {
    fn body(&self, cx: &mut Cx) -> Element {
        let many = cx.state(|| vec![0usize, 2]);
        let one = cx.state(|| 1usize);
        column((
            segmented_multiple(&["A", "B", "C"], &many.binding()).id("multi"),
            segmented_icons(&[("gear", "General"), ("folder", "Files")], &one.binding())
                .id("icons"),
            pane_control(&[("gear", "General"), ("folder", "Files")], &one.binding()).id("panes"),
        ))
    }
}

#[test]
fn multiple_icon_and_pane_segments_hold_their_selection() {
    let _rt = Headless::new().unwrap();
    let root = mount(host(), || Segs);
    let multi = root.host().find("multi").unwrap();
    assert_eq!(multi.get_int(Prop::STYLE).unwrap(), 1);
    let panes = root.host().find("panes").unwrap();
    assert_eq!(panes.get_int(Prop::SELECTED_INDEX).unwrap(), 1);
    assert!(root.host().find("icons").is_some());
}

struct Folding;

impl Component for Folding {
    fn body(&self, cx: &mut Cx) -> Element {
        let open = cx.state(|| true);
        let o2 = open.clone();
        column((
            button("Toggle")
                .id("toggle")
                .on_activate(move || o2.update(|o| *o = !*o)),
            sidebar(vec![
                sidebar_group(
                    "Favourites",
                    &open.binding(),
                    vec![
                        sidebar_row("clock", "Recents", false, || {})
                            .id("row")
                            .children(sidebar_badge("3")),
                    ],
                )
                .id("group"),
                sidebar_footer(|| {}, || {}).id("footer"),
            ]),
        ))
    }
}

#[test]
fn a_sidebar_group_folds_and_unfolds_its_rows() {
    let rt = Headless::new().unwrap();
    let root = mount(host(), || Folding);
    let group = root.host().find("group").unwrap();
    let row = root.host().find("row").unwrap();
    assert!(root.host().find("footer").is_some());
    assert!(group.get_bool(Prop::EXPANDED).unwrap());
    let toggle = root.host().find("toggle").unwrap();
    toggle.click().unwrap();
    for _ in 0..60 {
        rt.advance(0.02);
    }
    assert!(!group.get_bool(Prop::EXPANDED).unwrap());
    assert!(
        row.get_bool(Prop::HIDDEN).unwrap(),
        "folded rows leave the layout"
    );
    toggle.click().unwrap();
    for _ in 0..60 {
        rt.advance(0.02);
    }
    assert!(
        !row.get_bool(Prop::HIDDEN).unwrap(),
        "unfolded rows come back"
    );
}

#[test]
fn a_scroll_view_takes_its_mode_axis_and_hold() {
    let _rt = Headless::new().unwrap();
    struct Scroll;
    impl Component for Scroll {
        fn body(&self, _cx: &mut Cx) -> Element {
            scroll_view(label("x"))
                .id("s")
                .style(1)
                .scroll_axis(1)
                .scroll_hold(2)
        }
    }
    let root = mount(host(), || Scroll);
    let s = root.host().find("s").unwrap();
    assert_eq!(s.get_int(Prop::STYLE).unwrap(), 1);
    assert_eq!(s.get_int(Prop::VALUE).unwrap(), 1);
}

struct FieldOptions;

impl Component for FieldOptions {
    fn body(&self, cx: &mut Cx) -> Element {
        let text = cx.state(|| "abc".to_string());
        column((
            text_field("x", &text.binding())
                .id("f")
                .clearable(true)
                .error("Bad"),
            inline_button_field("y", &text.binding(), "Browse", || {}).id("inline"),
            combo_button(|| {}).id("cb"),
        ))
    }
}

#[test]
fn fields_take_clear_error_inline_button_and_combo_button() {
    let _rt = Headless::new().unwrap();
    let root = mount(host(), || FieldOptions);
    let f = root.host().find("f").unwrap();
    assert!(f.get_bool(Prop::SHOWS_CLEAR).unwrap_or(true));
    assert!(root.host().find("inline").is_some());
    assert!(root.host().find("cb").is_some());
}

struct Number;

impl Component for Number {
    fn body(&self, cx: &mut Cx) -> Element {
        let v = cx.state(|| 98.0f64);
        let b = v.binding();
        column((
            number_field(cx, &b, 0.0, 99.0, 1.0),
            label(format!("{}", v.get())).id("shown"),
        ))
    }
}

#[test]
fn a_number_field_steps_clamps_and_shows_the_value() {
    let rt = Headless::new().unwrap();
    let root = mount(host(), || Number);
    let field = root.host().find("Value field").unwrap();
    let stepper = root.host().find("Stepper").unwrap();
    assert_eq!(field.get_str(Prop::TEXT).unwrap(), "98");
    stepper.perform(3 /* AK_ACTION_INCREMENT */).unwrap();
    rt.advance(0.01);
    assert_eq!(
        root.host()
            .find("shown")
            .unwrap()
            .get_str(Prop::TEXT)
            .unwrap(),
        "99"
    );
    assert_eq!(field.get_str(Prop::TEXT).unwrap(), "99");
    // Past the maximum nothing moves.
    stepper.perform(3 /* AK_ACTION_INCREMENT */).unwrap();
    rt.advance(0.01);
    assert_eq!(field.get_str(Prop::TEXT).unwrap(), "99");
}
