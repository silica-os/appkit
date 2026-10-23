//! The view kinds `ak_view_new` makes (`AK_KIND_*`).

use appkit_sys::*;

use crate::amalfi::button::PushButton;
use crate::amalfi::buttons::{HelpButton, IconButton, LinkButton, MenuButton, ToolbarButton};
use crate::amalfi::collections::{GridView, ListView, TableHeader, TableView};
use crate::amalfi::disclosure::{Disclosure, DisclosureGroup};
use crate::amalfi::fields::{FieldKind, TextField};
use crate::amalfi::forms::{FormLayout, FormRow};
use crate::amalfi::group_box::GroupBox;
use crate::amalfi::label::{Label, SymbolImage};
use crate::amalfi::progress::{CircularProgress, ProgressBar, Spinner};
use crate::amalfi::scrollbars::ScrollView;
use crate::amalfi::segments::SegmentedControl;
use crate::amalfi::sidebars;
use crate::amalfi::sliders::{Dial, Slider};
use crate::amalfi::statusbar::StatusBar;
use crate::amalfi::steppers::Stepper;
use crate::amalfi::switch::Switch;
use crate::amalfi::titlebars::{Tab, TabBar, TitleBar};
use crate::amalfi::toggles::Toggle;
use crate::amalfi::wells::{ColorWell, ImageWell};
use crate::core::layout::{Alignment, Grid, Overlay, Spacer, Stack};
use crate::core::widget::{Empty, Widget};

/// A new widget of `kind`.
pub fn make(kind: u32) -> Option<Box<dyn Widget>> {
    Some(match kind {
        AK_KIND_VIEW => Box::new(Empty),
        AK_KIND_ROW => Box::new(Stack::row(8.0)),
        AK_KIND_COLUMN => Box::new(Stack::column(8.0)),
        AK_KIND_OVERLAY => Box::new(Overlay {
            alignment: Alignment::CENTER,
        }),
        AK_KIND_GRID => Box::new(Grid {
            columns: 2,
            column_spacing: 8.0,
            row_spacing: 8.0,
            alignment: Alignment::TOP_LEADING,
        }),
        AK_KIND_SPACER => Box::new(Spacer::default()),
        AK_KIND_LABEL => Box::new(Label::new("")),
        AK_KIND_SYMBOL => Box::new(SymbolImage::new("questionmark-circle")),
        AK_KIND_PUSH_BUTTON => Box::new(PushButton::new("")),
        AK_KIND_TEXT_VIEW => Box::new(crate::text::view::TextView::new(
            "",
            crate::text::TextStyle::default(),
        )),
        AK_KIND_CHECKBOX => Box::new(Toggle::checkbox()),
        AK_KIND_RADIO => Box::new(Toggle::radio()),
        AK_KIND_SWITCH => Box::new(Switch::new()),
        AK_KIND_SLIDER => Box::new(Slider::new()),
        AK_KIND_STEPPER => Box::new(Stepper::new()),
        AK_KIND_PROGRESS_BAR => Box::new(ProgressBar::new()),
        AK_KIND_CIRCULAR_PROGRESS => Box::new(CircularProgress::new(0.0)),
        AK_KIND_SPINNER => Box::new(Spinner::new()),
        AK_KIND_SEGMENTED => Box::new(SegmentedControl::new(Vec::new())),
        AK_KIND_TEXT_FIELD => Box::new(TextField::new(FieldKind::Text)),
        AK_KIND_SECURE_FIELD => Box::new(TextField::new(FieldKind::Secure)),
        AK_KIND_SEARCH_FIELD => Box::new(TextField::new(FieldKind::Search)),
        AK_KIND_TOKEN_FIELD => Box::new(TextField::new(FieldKind::Tokens)),
        AK_KIND_TEXT_AREA => Box::new(TextField::new(FieldKind::Area)),
        AK_KIND_COMBO_BOX => Box::new(TextField::new(FieldKind::Combo)),
        AK_KIND_POP_UP_BUTTON => Box::new(MenuButton::pop_up(&[], 0)),
        AK_KIND_PULL_DOWN_BUTTON => Box::new(MenuButton::pull_down("", &[])),
        AK_KIND_ICON_BUTTON => Box::new(IconButton::new("questionmark-circle")),
        AK_KIND_HELP_BUTTON => Box::new(HelpButton::default()),
        AK_KIND_LINK_BUTTON => Box::new(LinkButton::new("")),
        AK_KIND_TOOLBAR_BUTTON => Box::new(ToolbarButton::new("questionmark-circle")),
        AK_KIND_DISCLOSURE => Box::new(Disclosure::new()),
        AK_KIND_DISCLOSURE_GROUP => Box::new(DisclosureGroup::new("")),
        AK_KIND_GROUP_BOX => Box::new(GroupBox::new(None)),
        AK_KIND_COLOR_WELL => Box::new(ColorWell::new(crate::color::Color::rgb(0x0e, 0x77, 0xe6))),
        AK_KIND_IMAGE_WELL => Box::new(ImageWell::new()),
        AK_KIND_SCROLL_VIEW => Box::new(ScrollView::new()),
        AK_KIND_DIAL => Box::new(Dial::new()),
        AK_KIND_LIST => Box::new(ListView::new(0, 28.0, Box::new(|_, _| None))),
        AK_KIND_TABLE_HEADER => Box::new(TableHeader::new(Vec::new())),
        AK_KIND_SIDEBAR_ROW => Box::new(sidebars::Row::new("folder", "")),
        AK_KIND_SIDEBAR_HEADER => Box::new(sidebars::Header::new("")),
        AK_KIND_SIDEBAR_GROUP => Box::new(sidebars::Group::new(true)),
        AK_KIND_SIDEBAR_PANEL => Box::new(sidebars::Panel::default()),
        AK_KIND_FORM => Box::new(FormLayout::new(None)),
        AK_KIND_FORM_ROW => Box::new(FormRow::new("")),
        AK_KIND_TITLE_BAR => Box::new(TitleBar),
        AK_KIND_TAB_BAR => Box::new(TabBar),
        AK_KIND_TAB => Box::new(Tab::new("")),
        AK_KIND_STATUS_BAR => Box::new(StatusBar),
        AK_KIND_TEXT => Box::new(crate::amalfi::text::Text::new("")),
        AK_KIND_TABLE => Box::new(TableView::new(Vec::new())),
        AK_KIND_OUTLINE => Box::new(TableView::outline(Vec::new())),
        AK_KIND_GRID_VIEW => Box::new(GridView::new(0, Box::new(|_, _| None))),
        AK_KIND_TAB_VIEW => Box::new(crate::amalfi::tab_view::TabView::new(&[])),
        AK_KIND_STATUS_ITEM => Box::new(crate::amalfi::statusbar::StatusItem::new(
            crate::amalfi::statusbar::ItemContent::Glyph(String::new()),
        )),
        AK_KIND_TOOLBAR => Box::new(crate::amalfi::toolbar::Toolbar::default()),
        AK_KIND_TOOLBAR_GROUP => Box::new(crate::amalfi::segments::ActionSegments::new(Vec::new())),
        AK_KIND_FORM_VALUE => Box::new(crate::amalfi::forms::FormValue { expand: false }),
        AK_KIND_FORM_FOOTER => Box::new(crate::amalfi::forms::FormFooter::default()),
        AK_KIND_SIDEBAR_BADGE => Box::new(sidebars::Badge::new("")),
        AK_KIND_SIDEBAR_EJECT => Box::new(sidebars::EjectButton::default()),
        AK_KIND_SIDEBAR_FOOTER => Box::new(sidebars::Footer),
        AK_KIND_COMBO_BUTTON => Box::new(crate::amalfi::fields::ComboButton::default()),
        AK_KIND_VALUE_FIELD => Box::new(TextField::value()),
        AK_KIND_SWATCH_CHIP => Box::new(crate::amalfi::wells::SwatchChip::new(
            crate::color::Color::rgb(0, 0, 0),
        )),
        AK_KIND_PANE_CONTROL => Box::new(crate::amalfi::segments::PaneControl::new(Vec::new())),
        AK_KIND_BOX_SURFACE => Box::new(crate::amalfi::group_box::BoxSurface::default()),
        AK_KIND_GLASS_PANEL => Box::new(crate::amalfi::materials::GlassPanel::new(
            crate::amalfi::materials::Tier::Regular,
            16.0,
        )),
        AK_KIND_TOUCH_PROBE => Box::new(crate::amalfi::touchpad::TouchProbe::new()),
        AK_KIND_CALLOUT => Box::new(crate::amalfi::callouts::Callout::new(
            crate::amalfi::callouts::CalloutShape::new(
                Some(crate::amalfi::popovers::Edge::Bottom),
                crate::amalfi::popovers::Align::Center,
            ),
        )),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::a11y::{A11y, Role};

    #[test]
    fn every_control_describes_itself() {
        let containers = [
            AK_KIND_VIEW,
            AK_KIND_ROW,
            AK_KIND_COLUMN,
            AK_KIND_OVERLAY,
            AK_KIND_GRID,
            AK_KIND_SPACER,
            AK_KIND_SYMBOL,
            AK_KIND_SCROLL_VIEW,
            AK_KIND_GROUP_BOX,
            AK_KIND_SIDEBAR_GROUP,
            AK_KIND_FORM,
            AK_KIND_FORM_ROW,
            AK_KIND_TAB_VIEW,
            AK_KIND_FORM_VALUE,
            AK_KIND_FORM_FOOTER,
            AK_KIND_BOX_SURFACE,
            AK_KIND_GLASS_PANEL,
            AK_KIND_CALLOUT,
        ];
        let mut missing = Vec::new();
        for (name, kind) in appkit_sys::KINDS {
            let Some(w) = make(*kind) else {
                missing.push(format!("{name}: no widget"));
                continue;
            };
            let mut a = A11y::default();
            w.accessibility(&mut a);
            if a.role == Role::Group && !containers.contains(kind) {
                missing.push(format!("{name}: no role"));
            }
        }
        assert!(missing.is_empty(), "{missing:?}");
    }
}
