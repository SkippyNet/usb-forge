//! Visual style: a calm dark theme with a warm "forge" accent.

use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle};

pub const BG: Color32 = Color32::from_rgb(0x0f, 0x11, 0x17);
pub const PANEL: Color32 = Color32::from_rgb(0x15, 0x18, 0x21);
pub const CARD: Color32 = Color32::from_rgb(0x1b, 0x1f, 0x2a);
pub const CARD_HOVER: Color32 = Color32::from_rgb(0x22, 0x27, 0x34);
pub const BORDER: Color32 = Color32::from_rgb(0x2a, 0x30, 0x3e);
pub const TEXT: Color32 = Color32::from_rgb(0xe6, 0xe8, 0xee);
pub const MUTED: Color32 = Color32::from_rgb(0x8b, 0x93, 0xa7);
pub const ACCENT: Color32 = Color32::from_rgb(0xff, 0x7a, 0x45);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(0x5a, 0x2e, 0x1c);
pub const OK: Color32 = Color32::from_rgb(0x4c, 0xd1, 0x8a);
pub const WARN: Color32 = Color32::from_rgb(0xf5, 0xc4, 0x51);
pub const DANGER: Color32 = Color32::from_rgb(0xff, 0x5d, 0x6c);

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style.text_styles = [
            (
                TextStyle::Heading,
                FontId::new(20.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(14.5, FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(12.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(12.5, FontFamily::Monospace),
            ),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.interact_size.y = 28.0;

        let v = &mut style.visuals;
        v.panel_fill = BG;
        v.window_fill = PANEL;
        v.extreme_bg_color = Color32::from_rgb(0x0b, 0x0d, 0x12);
        v.faint_bg_color = CARD;
        v.override_text_color = Some(TEXT);
        v.hyperlink_color = ACCENT;
        v.selection.bg_fill = ACCENT_DIM;
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.window_corner_radius = CornerRadius::same(10);
        v.menu_corner_radius = CornerRadius::same(8);
        for w in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = CornerRadius::same(7);
        }
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        v.widgets.inactive.bg_fill = Color32::from_rgb(0x34, 0x3b, 0x4d);
        v.widgets.inactive.weak_bg_fill = CARD_HOVER;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x4a, 0x53, 0x68));
        v.widgets.inactive.fg_stroke = Stroke::new(1.5, TEXT);
        v.widgets.hovered.bg_fill = Color32::from_rgb(0x2c, 0x32, 0x42);
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x2c, 0x32, 0x42);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
        v.widgets.active.bg_fill = ACCENT_DIM;
        v.widgets.active.weak_bg_fill = ACCENT_DIM;
    });
}

/// A rounded card container.
pub fn card(fill: Color32, stroke: Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(12))
}

/// Numbered section heading, e.g. "① Choose a drive".
pub fn section(ui: &mut egui::Ui, number: &str, title: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 12.0, ACCENT);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            number,
            FontId::new(13.0, FontFamily::Proportional),
            BG,
        );
        ui.label(egui::RichText::new(title).size(17.0).strong());
    });
    ui.add_space(4.0);
}
