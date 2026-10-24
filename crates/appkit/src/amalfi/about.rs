//! The standard About panel (kit `Windows/Window/About`): the app's icon, name, version and
//! copyright from [`AppInfo`](crate::core::app::AppInfo), in a fixed 280 × 248 window whose
//! only working button is Close. The App menu's About item opens it; choosing it again brings
//! the open panel to the front.

use std::path::{Path, PathBuf};

use crate::amalfi::foundation::Label;
use crate::amalfi::tokens::{TypeStyle, typography};
use crate::color::Color;
use crate::core::a11y::{A11y, Role};
use crate::core::layout::Proposal;
use crate::core::ui::Ui;
use crate::core::widget::{LayoutCx, PaintCx, Widget};
use crate::core::window::{WindowId, WindowKind};
use crate::geom::{Rect, Size};
use crate::render::image::Image;
use crate::text::Align;

/// The panel's measures (kit `Windows/Window/About`).
pub mod metrics {
    /// The window.
    pub const WIDTH: f32 = 280.0;
    /// The window.
    pub const HEIGHT: f32 = 248.0;
    /// The band the window buttons are centred in: the close button sits at (16, 16).
    pub const BAR: f32 = 48.0;
    /// The app icon: 96 square at (92, 40).
    pub const ICON: f32 = 96.0;
    /// The app icon's top.
    pub const ICON_Y: f32 = 40.0;
    /// The texts' column.
    pub const TEXT_X: f32 = 24.0;
    /// The texts' column.
    pub const TEXT_WIDTH: f32 = 232.0;
    /// The name's top (Title 3).
    pub const NAME_Y: f32 = 152.0;
    /// The version's top (Caption, secondary).
    pub const VERSION_Y: f32 = 180.0;
    /// The copyright's top (Caption, tertiary).
    pub const COPYRIGHT_Y: f32 = 204.0;
}

/// The panel's content.
pub struct AboutPanel {
    icon: Option<Image>,
    name: String,
    version: String,
    copyright: String,
    labels: [Label; 3],
}

impl std::fmt::Debug for AboutPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AboutPanel")
            .field("icon", &self.icon.is_some())
            .field("name", &self.name)
            .field("version", &self.version)
            .field("copyright", &self.copyright)
            .finish()
    }
}

impl AboutPanel {
    /// The panel of the app `ui` describes.
    pub fn new(ui: &Ui) -> AboutPanel {
        let info = ui.app_info();
        let icon = if info.icon.is_empty() {
            app_icon_file(&info.id, &data_dirs())
        } else {
            Some(PathBuf::from(&info.icon))
        }
        .and_then(|p| decode(&p));
        AboutPanel {
            icon,
            name: info.name.clone(),
            version: if info.version.is_empty() {
                String::new()
            } else {
                format!("Version {}", info.version)
            },
            copyright: info.copyright.clone(),
            labels: Default::default(),
        }
    }
}

impl Widget for AboutPanel {
    fn kind(&self) -> &'static str {
        "AboutPanel"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::WIDTH, metrics::HEIGHT)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let r = cx.bounds();
        if let Some(icon) = &self.icon {
            let x = r.x + (r.width - metrics::ICON) / 2.0;
            let rect = Rect::new(x, r.y + metrics::ICON_Y, metrics::ICON, metrics::ICON);
            cx.rec.image("App icon", rect, icon.clone(), 1.0);
        }
        let c = cx.theme().colors;
        let lines: [(&str, &str, f32, &'static TypeStyle, Color); 3] = [
            (
                "App name",
                &self.name,
                metrics::NAME_Y,
                &typography::TITLE_3,
                c.text_primary,
            ),
            (
                "Version",
                &self.version,
                metrics::VERSION_Y,
                &typography::CAPTION,
                c.text_secondary,
            ),
            (
                "Copyright",
                &self.copyright,
                metrics::COPYRIGHT_Y,
                &typography::CAPTION,
                c.text_tertiary,
            ),
        ];
        for (cache, (name, text, y, token, color)) in self.labels.iter_mut().zip(lines) {
            if text.is_empty() {
                continue;
            }
            let style = cx.theme().text(token, color);
            let layout = cache.layout(text, &style, Align::Center, metrics::TEXT_WIDTH, Some(1));
            let height = token.line_height.unwrap_or(token.size);
            let rect = Rect::new(r.x + metrics::TEXT_X, r.y + y, metrics::TEXT_WIDTH, height);
            cx.rec.text(name, rect, layout, Some(token.name), color);
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
        a.label = Some(format!("About {}", self.name));
    }
}

impl Ui {
    /// Opens the About panel, or brings the open one to the front.
    pub fn show_about(&mut self) {
        if let Some(w) = self.about.filter(|w| self.window(*w).is_some()) {
            self.show_window(w);
            return;
        }
        let w = self.create_window(
            WindowKind::Normal,
            Size::new(metrics::WIDTH, metrics::HEIGHT),
        );
        let title = format!("About {}", self.app_info().name);
        self.set_title(w, &title);
        let size = Size::new(metrics::WIDTH, metrics::HEIGHT);
        self.set_size_limits(w, Some(size), Some(size));
        self.set_toolbar_height(w, metrics::BAR);
        let panel = AboutPanel::new(self);
        let v = self.create(panel);
        self.set_content(w, v);
        self.show_window(w);
        self.about = Some(w);
    }

    /// The open About panel's window.
    pub fn about_window(&self) -> Option<WindowId> {
        self.about.filter(|w| self.window(*w).is_some())
    }
}

/// The folders of `XDG_DATA_DIRS` (default `/usr/local/share:/usr/share`).
fn data_dirs() -> Vec<PathBuf> {
    let dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    dirs.split(':')
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// The largest icon of app `id` in the hicolor theme of `dirs`.
fn app_icon_file(id: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    if id.is_empty() || id.contains('/') {
        return None;
    }
    let sizes = [
        "512x512", "256x256", "scalable", "192x192", "128x128", "96x96",
    ];
    dirs.iter()
        .flat_map(|d| {
            sizes
                .iter()
                .map(move |s| d.join("icons/hicolor").join(s).join("apps"))
        })
        .flat_map(|d| ["png", "svg"].map(|ext| d.join(format!("{id}.{ext}"))))
        .find(|p| p.is_file())
}

/// The icon at `path`, drawn at twice its size in the panel for sharp HiDPI.
fn decode(path: &Path) -> Option<Image> {
    let data = std::fs::read(path).ok()?;
    if path.extension().is_some_and(|e| e == "svg") {
        Image::from_svg(&data, (metrics::ICON * 2.0) as u32)
    } else {
        Image::from_png(&data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::app::AppInfo;

    fn ui() -> Ui {
        let mut ui = Ui::new_headless();
        ui.set_app_info(AppInfo {
            id: "dev.silica.Test".into(),
            name: "Notes".into(),
            version: "1.0 (42)".into(),
            copyright: "Copyright © 2026 Silica.".into(),
            quit_enabled: true,
            ..AppInfo::default()
        });
        ui
    }

    #[test]
    fn the_panel_is_a_fixed_280_by_248_window() {
        let mut ui = ui();
        ui.show_about();
        let w = ui.about_window().expect("the panel is open");
        let win = ui.window(w).expect("its window");
        assert_eq!(win.size, Size::new(280.0, 248.0));
        assert!(!win.is_resizable());
        assert_eq!(win.toolbar_height, metrics::BAR);
        // Choosing About again brings the same panel back.
        ui.show_about();
        assert_eq!(ui.about_window(), Some(w));
        assert_eq!(ui.windows().count(), 1);
    }

    #[test]
    fn the_texts_come_from_the_app_info() {
        let ui = ui();
        let p = AboutPanel::new(&ui);
        assert_eq!(p.name, "Notes");
        assert_eq!(p.version, "Version 1.0 (42)");
        assert_eq!(p.copyright, "Copyright © 2026 Silica.");
    }

    #[test]
    fn the_icon_is_the_largest_in_hicolor() {
        let root = std::env::temp_dir().join(format!("appkit-about-{}", std::process::id()));
        let big = root.join("icons/hicolor/256x256/apps");
        let small = root.join("icons/hicolor/128x128/apps");
        for d in [&big, &small] {
            std::fs::create_dir_all(d).expect("test folder");
            std::fs::write(d.join("dev.silica.Test.png"), b"").expect("test file");
        }
        let found = app_icon_file("dev.silica.Test", std::slice::from_ref(&root));
        assert_eq!(found, Some(big.join("dev.silica.Test.png")));
        assert_eq!(app_icon_file("../x", std::slice::from_ref(&root)), None);
        let _ = std::fs::remove_dir_all(root);
    }
}
