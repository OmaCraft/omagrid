use super::{Color32, FontDefinitions, Tokens, contrast};
use crate::omacraft::ChromeFonts;
use crate::omarchy::{Palette, blend};

fn palette_id() -> egui::Id {
    egui::Id::unique("omagrid.appearance.palette")
}
fn fonts_id() -> egui::Id {
    egui::Id::unique("omagrid.appearance.fonts")
}

fn base_fonts_id() -> egui::Id {
    egui::Id::unique("omagrid.appearance.base-fonts")
}

/// Install a transient UI palette. No workbook data or persisted preference is changed.
/// Font and language rebuilds reuse this same context value through `Tokens::get` and `apply`.
pub fn set_omarchy_palette(ctx: &egui::Context, palette: Option<Palette>) {
    let previous = ctx.data(|data| data.get_temp::<Palette>(palette_id()));
    if previous == palette {
        return;
    }
    ctx.data_mut(|data| {
        if let Some(palette) = palette {
            data.insert_temp(palette_id(), palette);
        } else {
            data.remove::<Palette>(palette_id());
        }
    });
    let dark = palette.map(|p| p.dark).unwrap_or(ctx.theme() == egui::Theme::Dark);
    super::apply(ctx, dark);
    ctx.request_repaint();
}

pub(super) fn resolved(ctx: &egui::Context, dark: bool) -> Tokens {
    let base = if dark { Tokens::dark() } else { Tokens::light() };
    match ctx.data(|data| data.get_temp::<Palette>(palette_id())) {
        Some(palette) if palette.dark == dark => chrome(base, palette),
        _ => base,
    }
}

/// Only application chrome and interaction affordances change. Sheet/default ink, document
/// colours, chart styles and every `sheet_*` token retain their upstream values.
fn chrome(mut t: Tokens, p: Palette) -> Tokens {
    let text = readable(p.foreground, p.background, 4.5);
    t.window = p.background;
    t.ribbon = p.surface;
    t.menu_bg = p.surface;
    t.input_bg = p.background;
    t.text = readable(text, p.surface, 4.5);
    t.text_dim = readable(p.muted, p.surface, 4.5);
    t.text_disabled = blend(p.background, text, 0.45);
    t.accent = readable(p.accent, p.surface, 4.5);
    t.accent_dark = t.accent;
    t.accent_soft = p.selection;
    t.on_accent = readable(p.background, t.accent, 4.5);
    t.hover = blend(p.surface, t.text, 0.08);
    t.pressed = blend(p.surface, t.text, 0.14);
    t.separator = blend(p.surface, t.text, 0.22);
    t.ribbon_border = t.separator;
    t.input_border = t.separator;
    t.tab_bar = p.background;
    t.tab_active = p.surface;
    t.status_bar = p.background;
    t.header_bg = p.surface;
    t.header_text = t.text;
    t.header_line = t.separator;
    t.header_sel_bg = p.selection;
    t.header_sel_text = readable(t.text, p.selection, 4.5);
    t.header_all_bg = p.selection;
    // Selection on white paper needs its own contrasting ink, never a theme's dark-on-dark pair.
    t.sel_border = readable(p.accent, t.grid_bg, 3.0);
    t.sel_fill = t.sel_border.gamma_multiply(0.12);
    t.danger = readable(t.danger, p.surface, 4.5);
    t.paper = p.surface;
    for c in &mut t.cat {
        *c = readable(*c, p.surface, 3.0);
    }
    for c in &mut t.ref_colors {
        *c = readable(*c, p.background, 4.5);
    }
    t.syntax.keyword = readable(t.syntax.keyword, p.background, 4.5);
    t.syntax.string = readable(t.syntax.string, p.background, 4.5);
    t.syntax.number = readable(t.syntax.number, p.background, 4.5);
    t.syntax.comment = readable(t.syntax.comment, p.background, 4.5);
    t.syntax.builtin = readable(t.syntax.builtin, p.background, 4.5);
    t
}

fn readable(color: Color32, bg: Color32, ratio: f32) -> Color32 {
    if contrast(color, bg) >= ratio {
        return color;
    }
    let target = if contrast(Color32::WHITE, bg) > contrast(Color32::BLACK, bg) { Color32::WHITE } else { Color32::BLACK };
    for i in 1..=20 {
        let candidate = blend(color, target, i as f32 / 20.0);
        if contrast(candidate, bg) >= ratio {
            return candidate;
        }
    }
    target
}

pub(crate) fn set_omarchy_fonts(ctx: &egui::Context, fonts: ChromeFonts) -> bool {
    let previous = ctx.data(|d| d.get_temp::<ChromeFonts>(fonts_id())).unwrap_or_default();
    if previous == fonts {
        return false;
    }
    ctx.data_mut(|d| d.insert_temp(fonts_id(), fonts));
    true
}

/// Reuse the exact current document font set; a chrome refresh must never rerun discovery,
/// clear the bound-family registry, or briefly display workbook fallback faces.
pub(crate) fn refresh_omarchy_fonts(ctx: &egui::Context, fonts: ChromeFonts) -> bool {
    if ctx.data(|d| d.get_temp::<ChromeFonts>(fonts_id())).unwrap_or_default() == fonts {
        return false;
    }
    let Some(base) = ctx.data(|d| d.get_temp::<FontDefinitions>(base_fonts_id())) else { return false };
    set_omarchy_fonts(ctx, fonts);
    ctx.set_fonts(with_omarchy_fonts(ctx, base));
    true
}

/// Install the OmaCraft chrome fonts (bundled Inter, the detected system faces and the "Use
/// system font" switch) into UI roles only; CELL/MONO/SERIF, document-ui and workbook-specific
/// chains remain byte-for-byte intact.
pub(crate) fn with_omarchy_fonts(ctx: &egui::Context, mut fonts: FontDefinitions) -> FontDefinitions {
    ctx.data_mut(|d| d.insert_temp(base_fonts_id(), fonts.clone()));
    let chrome = ctx.data(|d| d.get_temp::<ChromeFonts>(fonts_id())).unwrap_or_default();
    crate::omacraft::install(&mut fonts, &chrome, &[]);
    fonts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{UI, UI_BOLD};
    use egui::FontFamily;

    #[test]
    fn palette_survives_reapply_and_never_changes_paper_tokens() {
        let ctx = egui::Context::default();
        super::super::apply(&ctx, false);
        let p = Palette::parse("mode='dark'\nbackground='#101020'\nforeground='#eeeedd'\naccent='#aabbcc'", false).unwrap();
        set_omarchy_palette(&ctx, Some(p));
        super::super::apply(&ctx, true);
        let t = Tokens::get(&ctx);
        assert_eq!(t.window, p.background);
        assert_eq!(ctx.global_style().visuals.panel_fill, t.window);
        let base = Tokens::dark();
        assert_eq!(
            (t.grid_bg, t.cell_text, t.sheet_ref_colors, t.sheet_control_bg),
            (base.grid_bg, base.cell_text, base.sheet_ref_colors, base.sheet_control_bg)
        );
        set_omarchy_palette(&ctx, None);
        assert_eq!(Tokens::get(&ctx), Tokens::dark());
    }

    #[test]
    fn unreadable_theme_foregrounds_get_safe_contrast() {
        let p = Palette::parse("background='#222222'\nforeground='#222222'\naccent='#222222'", false).unwrap();
        let t = chrome(Tokens::dark(), p);
        assert!(contrast(t.text, t.window) >= 4.5);
        assert!(contrast(t.text_dim, t.ribbon) >= 4.5);
        assert!(contrast(t.sel_border, t.grid_bg) >= 3.0);
    }

    /// A "detected" system face (an egui default face standing in), as UI stack and mono.
    fn system_chrome(base: &FontDefinitions, use_system_font: bool) -> ChromeFonts {
        let data = base.font_data.values().next().unwrap();
        let face = omacraft_ui::fonts::SystemFace {
            family: "Stand-in Sans".into(),
            path: "/usr/share/fonts/stand-in.ttf".into(),
            index: data.index,
            data: std::sync::Arc::new(data.bytes().to_vec()),
        };
        ChromeFonts {
            options: omacraft_ui::FontOptions { use_system_font },
            system: omacraft_ui::SystemFaces { ui: vec![face.clone()], ui_bold: vec![], mono: Some(face) },
        }
    }

    #[test]
    fn live_font_refresh_preserves_bound_workbook_and_document_faces_on_every_pass() {
        let _guard = crate::fonts::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let ctx = egui::Context::default();
        let mut base = super::super::pinned_font_definitions();
        let workbook_role = "omagrid-regression-workbook";
        let document = FontFamily::Name(super::super::DOCUMENT_UI.into());
        let document_bold = FontFamily::Name(super::super::DOCUMENT_UI_BOLD.into());
        // Document drawing keeps upstream's interface chain; only chrome gets Inter.
        assert_eq!(base.families[&FontFamily::Name(UI.into())][0], omacraft_ui::fonts::keys::INTER);
        for family in [&document, &document_bold] {
            assert!(!base.families[family].iter().any(|k| k.starts_with("omacraft/")), "{family:?}");
        }
        let workbook = FontFamily::Name(workbook_role.into());
        base.families.insert(workbook.clone(), base.families[&document].clone());
        crate::fonts::set_bound(base.families.keys().filter_map(|family| match family {
            FontFamily::Name(name) => Some(name.to_string()),
            _ => None,
        }));
        ctx.set_fonts(with_omarchy_fonts(&ctx, base.clone()));
        for desired in [system_chrome(&base, true), ChromeFonts::default()] {
            assert!(refresh_omarchy_fonts(&ctx, desired));
            let cached = ctx.data(|d| d.get_temp::<FontDefinitions>(base_fonts_id())).unwrap();
            assert_eq!(cached.families, base.families);
            assert!(crate::fonts::is_bound(workbook_role));
            for _ in 0..2 {
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                    for family in [&workbook, &document, &document_bold] {
                        ui.label(egui::RichText::new("Workbook chart header").font(egui::FontId::new(12.0, family.clone())));
                    }
                });
                out.textures_delta.clear();
                assert!(crate::fonts::is_bound(workbook_role));
            }
        }
    }

    #[test]
    fn ui_font_override_never_replaces_document_font_chains() {
        let _guard = crate::fonts::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let ctx = egui::Context::default();
        let original = super::super::pinned_font_definitions();
        let chrome = system_chrome(&original, true);
        assert!(set_omarchy_fonts(&ctx, chrome.clone()));
        assert!(!set_omarchy_fonts(&ctx, chrome));
        let changed = with_omarchy_fonts(&ctx, original.clone());
        let chrome_families = [FontFamily::Proportional, FontFamily::Monospace, FontFamily::Name(UI.into()), FontFamily::Name(UI_BOLD.into())];
        for (family, chain) in &original.families {
            if !chrome_families.contains(family) && !matches!(family, FontFamily::Name(n) if n.starts_with("omacraft.")) {
                assert_eq!(changed.families.get(family), Some(chain), "{family:?}");
            }
        }
        // "Use system font": the system face leads the UI chain, Inter follows; mono is the system mono.
        let ui = &changed.families[&FontFamily::Name(UI.into())];
        assert!(ui[0].starts_with("system/"), "{ui:?}");
        assert_eq!(ui[1], omacraft_ui::fonts::keys::INTER);
        assert!(changed.families[&FontFamily::Monospace][0].starts_with("system/"));
        assert!(set_omarchy_fonts(&ctx, ChromeFonts::default()));
        assert_eq!(with_omarchy_fonts(&ctx, original.clone()).families, original.families);
    }
}
