//! Builders for the Amalfi controls, bound to state: each sets the control's value from the
//! binding on every render and writes the user's changes back.

use std::rc::Rc;

use appkit_rs::{Kind, Prop, Signal, Value};

use crate::Binding;
use crate::element::{Element, PValue};

fn s(text: &str) -> PValue {
    PValue::Str(Rc::from(text))
}

fn list(items: &[&str]) -> PValue {
    PValue::Strings(
        items
            .iter()
            .map(|i| (*i).to_string())
            .collect::<Vec<_>>()
            .into(),
    )
}

/// A checkbox bound to a boolean.
pub fn checkbox(title: &str, value: &Binding<bool>) -> Element {
    let b = value.clone();
    Element::of(Kind::CHECKBOX)
        .prop(Prop::TITLE, s(title))
        .prop(Prop::STATE, PValue::Int(i64::from(value.get())))
        .on(Signal::VALUE_CHANGED, move |v| {
            if let Some(on) = v.as_bool() {
                b.set(on);
            }
        })
}

/// A radio button: `selected`, and `select` when the user chooses it.
pub fn radio(title: &str, selected: bool, select: impl Fn() + 'static) -> Element {
    Element::of(Kind::RADIO)
        .prop(Prop::TITLE, s(title))
        .prop(Prop::STATE, PValue::Int(i64::from(selected)))
        .on(Signal::VALUE_CHANGED, move |v| {
            if v.as_bool() == Some(true) {
                select();
            }
        })
}

/// A switch bound to a boolean.
pub fn switch(value: &Binding<bool>) -> Element {
    let b = value.clone();
    Element::of(Kind::SWITCH)
        .prop(Prop::VALUE, PValue::Bool(value.get()))
        .on(Signal::VALUE_CHANGED, move |v| {
            if let Some(on) = v.as_bool() {
                b.set(on);
            }
        })
}

/// A slider bound to a number in `min..=max`.
pub fn slider(value: &Binding<f64>, min: f64, max: f64) -> Element {
    let b = value.clone();
    Element::of(Kind::SLIDER)
        .prop(Prop::MIN_VALUE, PValue::Float(min))
        .prop(Prop::MAX_VALUE, PValue::Float(max))
        .prop(Prop::VALUE, PValue::Float(value.get()))
        .on(Signal::VALUE_CHANGED, move |v| {
            if let Some(x) = v.as_f64() {
                b.set(x);
            }
        })
}

/// A stepper bound to a number in `min..=max` by `step`.
pub fn stepper(value: &Binding<f64>, min: f64, max: f64, step: f64) -> Element {
    let b = value.clone();
    Element::of(Kind::STEPPER)
        .prop(Prop::MIN_VALUE, PValue::Float(min))
        .prop(Prop::MAX_VALUE, PValue::Float(max))
        .prop(Prop::STEP, PValue::Float(step))
        .prop(Prop::VALUE, PValue::Float(value.get()))
        .on(Signal::VALUE_CHANGED, move |v| {
            if let Some(x) = v.as_f64() {
                b.set(x);
            }
        })
}

/// A progress bar: `Some(0..=1)` determinate, `None` indeterminate.
pub fn progress(value: Option<f64>) -> Element {
    let e = Element::of(Kind::PROGRESS_BAR);
    match value {
        Some(v) => e.prop(Prop::VALUE, PValue::Float(v)),
        None => e.prop(Prop::INDETERMINATE, PValue::Bool(true)),
    }
}

/// A spinner.
pub fn spinner() -> Element {
    Element::of(Kind::SPINNER)
}

/// A segmented control of text segments bound to the selected index.
pub fn segmented(items: &[&str], selected: &Binding<usize>) -> Element {
    let b = selected.clone();
    Element::of(Kind::SEGMENTED)
        .prop(Prop::ITEMS, list(items))
        .prop(Prop::SELECTED_INDEX, PValue::Int(selected.get() as i64))
        .on(Signal::VALUE_CHANGED, move |v| {
            if let Some(i) = v.as_int().and_then(|i| usize::try_from(i).ok()) {
                b.set(i);
            }
        })
}

/// A segmented control that allows several segments on: `selected` holds their indices.
pub fn segmented_multiple(items: &[&str], selected: &Binding<Vec<usize>>) -> Element {
    let b = selected.clone();
    let rows: Vec<String> = selected.get().iter().map(usize::to_string).collect();
    Element::of(Kind::SEGMENTED)
        .prop(Prop::ITEMS, list(items))
        .prop(Prop::STYLE, PValue::Int(1))
        .prop(Prop::SELECTED_ROWS, PValue::Strings(rows.into()))
        .on(Signal::VALUE_CHANGED, move |v| {
            if let Value::List(l) = v {
                b.set(l.iter().filter_map(|s| s.parse().ok()).collect());
            }
        })
}

/// A segmented control of icon segments: `items` are (symbol, accessible label).
pub fn segmented_icons(items: &[(&str, &str)], selected: &Binding<usize>) -> Element {
    let labels: Vec<&str> = items.iter().map(|i| i.1).collect();
    let symbols: Vec<&str> = items.iter().map(|i| i.0).collect();
    segmented(&labels, selected).prop(Prop::ITEM_SYMBOLS, list(&symbols))
}

/// The Icon + Label control that switches the panes of a window: `items` are (symbol, label).
pub fn pane_control(items: &[(&str, &str)], selected: &Binding<usize>) -> Element {
    let b = selected.clone();
    let labels: Vec<&str> = items.iter().map(|i| i.1).collect();
    let symbols: Vec<&str> = items.iter().map(|i| i.0).collect();
    Element::of(Kind::PANE_CONTROL)
        .prop(Prop::ITEMS, list(&labels))
        .prop(Prop::ITEM_SYMBOLS, list(&symbols))
        .prop(Prop::SELECTED_INDEX, PValue::Int(selected.get() as i64))
        .on(Signal::VALUE_CHANGED, move |v| {
            if let Some(i) = v.as_int().and_then(|i| usize::try_from(i).ok()) {
                b.set(i);
            }
        })
}

fn field(kind: Kind, placeholder: &str, text: &Binding<String>) -> Element {
    let b = text.clone();
    Element::of(kind)
        .prop(Prop::PLACEHOLDER, s(placeholder))
        .prop(Prop::TEXT, s(&text.get()))
        .on(Signal::TEXT_CHANGED, move |v| {
            if let Value::Str(t) = v
                && *t != b.get()
            {
                b.set(t.clone());
            }
        })
}

/// A text field bound to a string.
pub fn text_field(placeholder: &str, text: &Binding<String>) -> Element {
    field(Kind::TEXT_FIELD, placeholder, text)
}

/// A combo box: a text field bound to a string, with `items` to pick from.
pub fn combo_box(placeholder: &str, items: &[&str], text: &Binding<String>) -> Element {
    field(Kind::COMBO_BOX, placeholder, text).prop(Prop::ITEMS, list(items))
}

/// A secure (password) field bound to a string.
pub fn secure_field(placeholder: &str, text: &Binding<String>) -> Element {
    field(Kind::SECURE_FIELD, placeholder, text)
}

/// A search field bound to a string.
pub fn search_field(placeholder: &str, text: &Binding<String>) -> Element {
    field(Kind::SEARCH_FIELD, placeholder, text)
}

/// A multi-line text area bound to a string (short texts; documents use `text_view`).
pub fn text_area(placeholder: &str, text: &Binding<String>) -> Element {
    field(Kind::TEXT_AREA, placeholder, text)
}

/// A pop-up button bound to the selected index.
pub fn pop_up(items: &[&str], selected: &Binding<usize>) -> Element {
    let b = selected.clone();
    Element::of(Kind::POP_UP_BUTTON)
        .prop(Prop::ITEMS, list(items))
        .prop(Prop::SELECTED_INDEX, PValue::Int(selected.get() as i64))
        .on(Signal::SELECTION_CHANGED, move |v| {
            if let Some(i) = v.as_int().and_then(|i| usize::try_from(i).ok()) {
                b.set(i);
            }
        })
}

/// A pull-down button: `chosen` gets the index of the item picked.
pub fn pull_down(title: &str, items: &[&str], chosen: impl Fn(usize) + 'static) -> Element {
    Element::of(Kind::PULL_DOWN_BUTTON)
        .prop(Prop::TITLE, s(title))
        .prop(Prop::ITEMS, list(items))
        .on(Signal::SELECTION_CHANGED, move |v| {
            if let Some(i) = v.as_int().and_then(|i| usize::try_from(i).ok()) {
                chosen(i);
            }
        })
}

/// A round icon button.
pub fn icon_button(symbol: &str) -> Element {
    Element::of(Kind::ICON_BUTTON).symbol(symbol)
}

/// A toolbar button.
pub fn toolbar_button(symbol: &str) -> Element {
    Element::of(Kind::TOOLBAR_BUTTON).symbol(symbol)
}

/// A toolbar's glass capsule of icon segments (back and forward, an item group): `items`
/// are (symbol, accessible label); `action` gets the index of the segment pressed.
pub fn toolbar_group(items: &[(&str, &str)], action: impl Fn(usize) + 'static) -> Element {
    let symbols: Vec<&str> = items.iter().map(|i| i.0).collect();
    let labels: Vec<&str> = items.iter().map(|i| i.1).collect();
    Element::of(Kind::TOOLBAR_GROUP)
        .prop(Prop::ITEM_SYMBOLS, list(&symbols))
        .prop(Prop::ITEMS, list(&labels))
        .on(Signal::ACTIVATED, move |v| {
            if let Value::Int(i) = v
                && let Ok(i) = usize::try_from(*i)
            {
                action(i);
            }
        })
}

/// A toolbar's view switcher (ui-kit `STitlebarPiece` `views`): icon segments, one selected,
/// 36 tall; `items` are (symbol, accessible label).
pub fn toolbar_views(items: &[(&str, &str)], selected: &Binding<usize>) -> Element {
    let symbols: Vec<&str> = items.iter().map(|i| i.0).collect();
    let labels: Vec<&str> = items.iter().map(|i| i.1).collect();
    segmented(&labels, selected).prop(Prop::ITEM_SYMBOLS, list(&symbols))
}

/// A toolbar's title (kit `Piece/Title`: Headline, 8 in).
pub fn toolbar_title(title: &str) -> Element {
    crate::label(title)
        .text_style("Headline")
        .padding_xy(4.0, 0.0)
}

/// A toolbar's search capsule (kit `Piece/Search`: 200 wide).
pub fn toolbar_search(placeholder: &str, text: &Binding<String>) -> Element {
    search_field(placeholder, text).width(200.0)
}

/// A window's toolbar of `items` (toolbar buttons, groups, a title, a search, `spacer()`s
/// sharing the free width), laid out as the kit's pieces after the window controls. See
/// [`Element::toolbar`].
pub fn toolbar(items: impl crate::IntoChildren) -> Element {
    Element::of(Kind::TOOLBAR).children(items).fill_width()
}

/// A window split into a full-height sidebar and its content (ui-kit `SWindow` with a split
/// toolbar). `sidebar_bar` are the items of the section above the sidebar (after the window
/// controls; a hide button closes it), `bar` those of the content's section; `open` holds
/// whether the sidebar is shown, and `width` is its width (the grip on its edge resizes it).
/// The window's content becomes full size, as with [`Element::toolbar`].
pub fn split_window(
    open: &Binding<bool>,
    width: f32,
    sidebar_bar: impl crate::IntoChildren,
    sidebar: Element,
    bar: impl crate::IntoChildren,
    content: Element,
) -> Element {
    let toggle = |b: &Binding<bool>, show: bool, label: &str| {
        let b = b.clone();
        toolbar_button("sidebar")
            .tooltip(label)
            .accessibility_label(label)
            .on_activate(move || b.set(show))
    };
    let mut side_items = sidebar_bar.into_children();
    side_items.push(crate::spacer());
    side_items.push(toggle(open, false, "Hide Sidebar"));
    let side = crate::column((
        toolbar(side_items).prop(Prop::STYLE, PValue::Int(2)),
        sidebar.fill(),
    ))
    .spacing(0.0)
    .alignment(0)
    .fill()
    .background_role("Surface/Sidebar");
    let main = crate::column((
        toolbar(bar).prop(Prop::STYLE, PValue::Int(1)),
        content.fill(),
    ))
    .spacing(0.0)
    .alignment(0)
    .fill();
    // The grip only needs to be hit: a clear fill makes it so.
    let grip = Element::of(Kind::VIEW).background(appkit_rs::rgba(0));
    let b = open.clone();
    let mut e = Element::of(Kind::SPLIT_WINDOW)
        .prop(Prop::EXPANDED, PValue::Bool(open.get()))
        .prop(Prop::VALUE, PValue::Float(f64::from(width)))
        .on(Signal::EXPANDED_CHANGED, move |v| {
            if let Some(o) = v.as_bool() {
                b.set(o);
            }
        })
        .children(vec![side, main, toggle(open, true, "Show Sidebar"), grip])
        .fill();
    e.effects.toolbar = Some(52.0);
    e
}

/// A form (kit `Forms/Layout`): a window-coloured panel with a title and its parts (sections,
/// then the footer) 20 apart.
pub fn form(title: Option<&str>, parts: impl crate::IntoChildren) -> Element {
    let e = Element::of(Kind::FORM);
    match title {
        Some(t) => e.prop(Prop::TITLE, s(t)),
        None => e,
    }
    .children(parts)
}

/// A form section (kit `Forms/Section`): a titled group box of rows 12 apart.
pub fn form_section(title: Option<&str>, rows: impl crate::IntoChildren) -> Element {
    let surface = Element::of(Kind::BOX_SURFACE)
        .prop(Prop::SPACING, PValue::Float(12.0))
        .id("Group")
        .children(rows);
    group_box(title, surface)
}

/// A form row (kit `Forms/Row`): `label` right-aligned in the label column, then `control`.
pub fn form_row(label: &str, control: Element) -> Element {
    form_row_with(label, control, None, false)
}

/// A form row with a help (or error) message under the control; `expand` makes the control
/// fill the value column (text fields, labelled switches).
pub fn form_row_with(
    label: &str,
    control: Element,
    message: Option<(&str, bool)>,
    expand: bool,
) -> Element {
    let mut value = Element::of(Kind::FORM_VALUE)
        .prop(Prop::EXPANDED, PValue::Bool(expand))
        .children(control);
    if let Some((text, error)) = message {
        let color = if error {
            "Text/Secondary"
        } else {
            "Text/Tertiary"
        };
        value = value.children(
            crate::label(text)
                .text_style("Caption")
                .color(color)
                .prop(Prop::WRAPS, PValue::Bool(true)),
        );
    }
    Element::of(Kind::FORM_ROW)
        .prop(Prop::TITLE, s(label))
        .children(value)
}

/// A form's footer (kit `Forms/Footer`): its buttons at the end, 8 apart.
pub fn form_footer(buttons: impl crate::IntoChildren) -> Element {
    Element::of(Kind::FORM_FOOTER).children(buttons)
}

/// A help button.
pub fn help_button() -> Element {
    Element::of(Kind::HELP_BUTTON)
}

/// A link.
pub fn link(title: &str) -> Element {
    Element::of(Kind::LINK_BUTTON).prop(Prop::TITLE, s(title))
}

/// A disclosure group: a titled row that expands to show `content`.
pub fn disclosure_group(title: &str, expanded: &Binding<bool>, content: Element) -> Element {
    let b = expanded.clone();
    Element::of(Kind::DISCLOSURE_GROUP)
        .prop(Prop::TITLE, s(title))
        .prop(Prop::EXPANDED, PValue::Bool(expanded.get()))
        .on(Signal::EXPANDED_CHANGED, move |v| {
            if let Some(e) = v.as_bool() {
                b.set(e);
            }
        })
        .children(content)
}

/// A group box around `content`.
pub fn group_box(title: Option<&str>, content: Element) -> Element {
    let e = Element::of(Kind::GROUP_BOX);
    match title {
        Some(t) => e.prop(Prop::TITLE, s(t)),
        None => e,
    }
    .children(content)
}

/// A colour well bound to a colour.
pub fn color_well(color: &Binding<appkit_rs::Color>) -> Element {
    let b = color.clone();
    Element::of(Kind::COLOR_WELL)
        .prop(Prop::COLOR_VALUE, PValue::Color(color.get()))
        .on(Signal::VALUE_CHANGED, move |v| {
            if let Value::Color(c) = v {
                b.set(*c);
            }
        })
}

/// A scroll view around `content`.
pub fn scroll_view(content: Element) -> Element {
    Element::of(Kind::SCROLL_VIEW).children(content)
}

/// A glass material: 0 Regular, 1 Clear.
pub type Glass = i64;

/// A glass panel of `tier` with corners of `radius` around `content`.
pub fn glass_panel(tier: Glass, radius: f32, content: Element) -> Element {
    Element::of(Kind::GLASS_PANEL)
        .prop(Prop::STYLE, PValue::Int(tier))
        .prop(Prop::VALUE, PValue::Float(f64::from(radius)))
        .children(content)
}

/// The touchpad probe: a panel showing the touchpad events it receives.
pub fn touch_probe() -> Element {
    Element::of(Kind::TOUCH_PROBE)
}

/// A callout around `content`: a glass bubble whose arrow is on `edge` (0 top, 1 bottom,
/// 2 leading, 3 trailing, anything else none).
pub fn callout(edge: i64, tier: Glass, content: Element) -> Element {
    Element::of(Kind::CALLOUT)
        .prop(Prop::EDGE, PValue::Int(edge))
        .prop(Prop::STYLE, PValue::Int(tier))
        .children(content)
}

struct ColorPanelMark;
struct FilePanelMark;

/// The colour panel's content over `colors` (0xRRGGBB) in rows of `columns`; `chosen` gets
/// the picked colour.
pub fn color_panel(
    colors: &[u32],
    columns: usize,
    swatches_only: bool,
    color: &Binding<appkit_rs::Color>,
) -> Element {
    let list: Vec<u32> = colors.to_vec();
    let sel = color.get();
    let selected = (u32::from(sel.r) << 16) | (u32::from(sel.g) << 8) | u32::from(sel.b);
    let flags = if swatches_only {
        appkit_rs::panel_flags::SWATCHES_ONLY
    } else {
        0
    };
    let b = color.clone();
    crate::element::made::<ColorPanelMark>(move || {
        appkit_rs::View::color_panel(&list, columns, Some(selected), flags).ok()
    })
    .on(Signal::VALUE_CHANGED, move |v| {
        if let Value::Color(c) = v {
            b.set(*c);
        }
    })
}

/// An open or save panel over `files`. `expanded` is the form; the panel's chevrons ask
/// for the other one through `set_expanded`, and its buttons call `on_action` with
/// "primary", "secondary", "new-folder", "back" or "forward".
pub fn file_panel(
    open: bool,
    expanded: &Binding<bool>,
    name: &str,
    files: &[&str],
    on_action: impl Fn(&str) + 'static,
) -> Element {
    let files: Vec<String> = files.iter().map(|f| (*f).to_string()).collect();
    let name = name.to_string();
    let exp = expanded.get();
    let flags = if open {
        appkit_rs::panel_flags::OPEN
    } else {
        0
    } | if exp || open {
        appkit_rs::panel_flags::EXPANDED
    } else {
        0
    };
    let b = expanded.clone();
    crate::element::made::<FilePanelMark>(move || {
        let refs: Vec<&str> = files.iter().map(String::as_str).collect();
        appkit_rs::View::file_panel(flags, &name, &refs).ok()
    })
    // A different form is a different panel: the key makes the reconciler build it again.
    .key(i64::from(exp))
    .on(Signal::EXPANDED_CHANGED, move |v| {
        if let Value::Bool(e) = v {
            b.set(*e);
        }
    })
    .on(Signal::ACTIVATED, move |v| {
        if let Value::Str(s) = v {
            on_action(s);
        }
    })
}

/// A sidebar row with a `symbol` and a `title`; `select` runs when it is pressed.
pub fn sidebar_row(
    symbol: &str,
    title: &str,
    selected: bool,
    select: impl Fn() + 'static,
) -> Element {
    Element::of(Kind::SIDEBAR_ROW)
        .prop(Prop::SYMBOL, s(symbol))
        .prop(Prop::TITLE, s(title))
        .prop(Prop::STATE, PValue::Bool(selected))
        .on(Signal::ACTIVATED, move |_| select())
}

/// A sidebar: the surface of the rows in `rows`.
pub fn sidebar(rows: impl crate::IntoChildren) -> Element {
    Element::of(Kind::SIDEBAR_PANEL).children(rows)
}

/// A count badge at the end of a sidebar row (a child of [`sidebar_row`]).
pub fn sidebar_badge(count: &str) -> Element {
    Element::of(Kind::SIDEBAR_BADGE).prop(Prop::TEXT, s(count))
}

/// The eject button of a volume row (a child of [`sidebar_row`]); `eject` runs on a click.
pub fn sidebar_eject(eject: impl Fn() + 'static) -> Element {
    Element::of(Kind::SIDEBAR_EJECT).on(Signal::ACTIVATED, move |_| eject())
}

/// A sidebar group: a header with a disclosure chevron over `rows`, which fold away when
/// the header is clicked. `open` holds whether the group is unfolded.
pub fn sidebar_group(title: &str, open: &Binding<bool>, rows: impl crate::IntoChildren) -> Element {
    let b = open.clone();
    let header = Element::of(Kind::SIDEBAR_HEADER)
        .prop(Prop::TITLE, s(title))
        .prop(Prop::STYLE, PValue::Int(1))
        .prop(Prop::EXPANDED, PValue::Bool(open.get()))
        .on(Signal::EXPANDED_CHANGED, move |v| {
            if let Some(o) = v.as_bool() {
                b.set(o);
            }
        });
    let mut kids = vec![header];
    kids.extend(rows.into_children());
    Element::of(Kind::SIDEBAR_GROUP)
        .prop(Prop::EXPANDED, PValue::Bool(open.get()))
        .children(kids)
}

/// The bar at the bottom of a sidebar with an add and a settings button.
pub fn sidebar_footer(add: impl Fn() + 'static, settings: impl Fn() + 'static) -> Element {
    let button = |symbol: &str, title: &str, f: Box<dyn Fn()>| {
        Element::of(Kind::ICON_BUTTON)
            .symbol(symbol)
            .prop(Prop::TITLE, s(title))
            .prop(Prop::STYLE, PValue::Int(1))
            .prop(Prop::CONTROL_SIZE, PValue::Int(1))
            .on(Signal::ACTIVATED, move |_| f())
    };
    Element::of(Kind::SIDEBAR_FOOTER).children(vec![
        button("plus", "Add", Box::new(add)),
        button("gear", "Settings", Box::new(settings)),
    ])
}

/// A text field with a small borderless button inside its trailing edge (a path with
/// "Browse"); `action` runs on the button.
pub fn inline_button_field(
    placeholder: &str,
    text: &Binding<String>,
    button_title: &str,
    action: impl Fn() + 'static,
) -> Element {
    text_field(placeholder, text).children(
        crate::button(button_title)
            .style(3)
            .control_size(1)
            .on_activate(action),
    )
}

/// The round chevron button of a combo box on its own (the kit's combo button).
pub fn combo_button(action: impl Fn() + 'static) -> Element {
    Element::of(Kind::COMBO_BUTTON).on(Signal::ACTIVATED, move |_| action())
}

fn format_number(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        v.to_string()
    }
}

/// A number field: the value field with a stepper beside it (the kit's `Steppers/With Field`),
/// 8 apart. `value` is clamped to `min..=max`; typing a number is committed on Return or when
/// the field loses the focus (a text that is no number gives the value back), and Up and Down
/// in the field step by `step`.
pub fn number_field(
    cx: &mut crate::Cx,
    value: &Binding<f64>,
    min: f64,
    max: f64,
    step: f64,
) -> Element {
    let (lo, hi) = (min.min(max), max.max(min));
    let typed = cx.state(|| format_number(value.get()));
    let seen = cx.state(|| value.get());
    if (seen.get() - value.get()).abs() > f64::EPSILON {
        // Changed from outside: the field shows it.
        seen.set(value.get());
        typed.set(format_number(value.get()));
    }
    let commit = {
        let (value, typed) = (value.clone(), typed.clone());
        move |text: &str| {
            let n = text
                .trim()
                .replace(',', ".")
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite())
                .map_or(value.get(), |n| n.clamp(lo, hi));
            value.set(n);
            typed.set(format_number(n));
        }
    };
    let (c1, c2, t1) = (commit.clone(), commit, typed.clone());
    let (v1, t2, v2, t3) = (value.clone(), typed.clone(), value.clone(), typed.clone());
    let field = Element::of(Kind::VALUE_FIELD)
        .id("Value field")
        .prop(Prop::TEXT, s(&typed.get()))
        .on(Signal::TEXT_CHANGED, move |v| {
            if let Value::Str(t) = v
                && *t != t1.get()
            {
                t1.set(t.clone());
            }
        })
        .on(Signal::SUBMITTED, move |v| {
            if let Value::Str(t) = v {
                c1(t);
            }
        })
        .on(Signal::EDITING_ENDED, {
            let typed = typed.clone();
            move |_| c2(&typed.get())
        })
        .on(Signal::ACTIVATED, move |v| {
            // Up (1) and Down (-1) in the field.
            let dir = v.as_int().unwrap_or(0).signum() as f64;
            let n = (v1.get() + dir * step).clamp(lo, hi);
            v1.set(n);
            t2.set(format_number(n));
        });
    let stepper = stepper(value, lo, hi, step)
        .id("Stepper")
        .on(Signal::VALUE_CHANGED, move |v| {
            if let Some(n) = v.as_f64() {
                v2.set(n);
                t3.set(format_number(n));
            }
        });
    crate::row((field, stepper)).spacing(8.0).alignment(1)
}

/// A swatch chip: a round colour with the selection ring when `selected`; `pick` runs on a click.
pub fn swatch_chip(color: appkit_rs::Color, selected: bool, pick: impl Fn() + 'static) -> Element {
    Element::of(Kind::SWATCH_CHIP)
        .prop(Prop::COLOR_VALUE, PValue::Color(color))
        .prop(Prop::STATE, PValue::Bool(selected))
        .on(Signal::ACTIVATED, move |_| pick())
}

/// A row of swatch chips (8 apart, 4 around) over `colors`; `selected` holds the chosen index.
pub fn swatch_row(colors: &[appkit_rs::Color], selected: &Binding<Option<usize>>) -> Element {
    let chips: Vec<Element> = colors
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let b = selected.clone();
            swatch_chip(*c, selected.get() == Some(i), move || b.set(Some(i))).key(i as i64)
        })
        .collect();
    crate::row(chips).spacing(8.0).padding(4.0)
}
