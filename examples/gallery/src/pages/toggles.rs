//! TogglesDoc: Try them, Checkbox, Radio, Switch.

use appkit_ui::prelude::*;

use super::{code, h2, lead, matrix, page, pick, stage};

/// The page: `only` picks the sections (1 checkboxes, 2 radio buttons, 3 switches).
pub struct Page(pub &'static [usize]);

impl Component for Page {
    fn body(&self, cx: &mut Cx) -> Element {
        let on = cx.state(|| true);
        let (a, b) = (cx.state(|| true), cx.state(|| false));
        let choice = cx.state(|| 0usize);
        let all = a.get() && b.get();
        let (a2, b2) = (a.clone(), b.clone());
        let select_all = checkbox(
            "Select all",
            &Binding::new(
                move || all,
                move |v| {
                    a2.set(v);
                    b2.set(v);
                },
            ),
        );
        let radios = |sel: usize, disabled: bool| {
            column(
                ["Sort by name", "Sort by date", "Sort by size"]
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        let c = choice.clone();
                        radio(t, sel == i, move || c.set(i))
                            .disabled(disabled)
                            .key(i as i64)
                    })
                    .collect::<Vec<_>>(),
            )
            .spacing(8.0)
            .alignment(0)
        };
        let try_them = column((
            lead("Toggles. The checkbox and radio are native inputs (keyboard, forms, labels); the switch is a role=switch button you can also drag."),
            h2("Try them"),
            stage(
                row((
                    column((
                        switch(&on.binding()),
                        label(format!("switch is {} · drag it", if on.get() { "on" } else { "off" }))
                            .color("Text/Secondary"),
                    ))
                    .spacing(8.0),
                    column((select_all, checkbox("First item", &a.binding()), checkbox("Second item", &b.binding())))
                        .spacing(8.0)
                        .alignment(0),
                    radios(choice.get(), false),
                ))
                .spacing(40.0)
                .alignment(0),
            ),
            code("checkbox(\"Label\", &checked)  radio(\"Sort by name\", selected, || …)"),
        ))
        .spacing(14.0)
        .alignment(0);
        let (c1, c2, c3) = (cx.state(|| true), cx.state(|| false), cx.state(|| true));
        let checks = column((
            h2("Checkbox"),
            matrix(
                &["Off", "On", "Disabled", "Disabled on"],
                vec![(
                    "Checkbox",
                    vec![
                        checkbox("Label", &c2.binding()),
                        checkbox("Label", &c1.binding()),
                        checkbox("Label", &c2.binding()).disabled(true),
                        checkbox("Label", &c3.binding()).disabled(true),
                    ],
                )],
            )
            .spacing(12.0),
        ))
        .spacing(10.0)
        .alignment(0);
        let rad = column((
            h2("Radio"),
            matrix(
                &["Rest", "Disabled"],
                vec![(
                    "Radio",
                    vec![radios(choice.get(), false), radios(choice.get(), true)],
                )],
            )
            .spacing(12.0),
        ))
        .spacing(10.0)
        .alignment(0);
        let sw = |i: i64, v: &Binding<bool>| switch(v).control_size(i);
        let s = cx.state(|| true);
        let o = cx.state(|| false);
        let switches = column((
            h2("Switch"),
            matrix(
                &["Off", "On", "Held (lens)", "Disabled"],
                ["mini", "small", "regular"]
                    .iter()
                    .enumerate()
                    .map(|(i, n)| {
                        (
                            *n,
                            vec![
                                sw(i as i64, &o.binding()),
                                sw(i as i64, &s.binding()),
                                sw(i as i64, &s.binding()).dragging(true),
                                sw(i as i64, &s.binding()).disabled(true),
                            ],
                        )
                    })
                    .collect(),
            )
            .spacing(12.0),
        ))
        .spacing(10.0)
        .alignment(0);
        page(pick(self.0, vec![try_them, checks, rad, switches]))
    }
}
