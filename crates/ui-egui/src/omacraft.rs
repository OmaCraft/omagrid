//! OmaCraft suite chrome: a thin adapter over the shared `omacraft-ui` crate.
//!
//! - Fonts: bundled Inter for chrome (`ui`/`ui-bold` and egui's `Proportional`), Inter Tabular for
//!   numbers that must line up, Instrument Sans for headings, and the system mono only for the
//!   formula bar and code (`Monospace`). "Use system font" (off by default) puts the system UI
//!   face in front of Inter.
//! - Type scale: [`Role`] tokens for menus, dialogs, settings and captions.
//!
//! The logic lives in `omacraft-ui`; this file only maps it onto GridCraft's font families, so
//! upstream files change by one call each. Document fonts (`cell*`, `serif`, `mono`, `math*`,
//! `document-ui*` and workbook families) are never touched.

use egui::{FontDefinitions, FontFamily, FontId};
pub use omacraft_ui::{Face, FontOptions, Role, SystemFaces};

use crate::theme::{UI, UI_BOLD};

/// What the chrome fonts are built from: the user's switch and what the system offers. The
/// appearance worker detects [`SystemFaces`]; an unchanged value never rebuilds the font set.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChromeFonts {
    pub options: FontOptions,
    pub system: SystemFaces,
}

/// Install the suite faces into an app font set. `scripts` are the app's script fallback keys
/// (CJK, Thai, Hebrew…), placed right after Inter and the system UI stack; the previous
/// `Proportional`/`Monospace` chains stay behind them, so no glyph is lost. Applying twice is
/// harmless: keys are deduplicated.
pub(crate) fn install(fonts: &mut FontDefinitions, chrome: &ChromeFonts, scripts: &[String]) {
    let scripts: Vec<&str> = scripts.iter().map(String::as_str).filter(|k| fonts.font_data.contains_key(*k)).collect();
    omacraft_ui::fonts::apply(fonts, chrome.options, &chrome.system, &scripts);
    // GridCraft's chrome draws through the named `ui`/`ui-bold` families.
    for (role, face) in [(UI, Face::Ui), (UI_BOLD, Face::UiStrong)] {
        if let Some(chain) = fonts.families.get(&face.family()).cloned() {
            fonts.families.insert(FontFamily::Name(role.into()), chain);
        }
    }
}

/// Register the type scale in a style. GridCraft keeps its own interactive row height (22 pt,
/// shared with the ribbon and grid controls); only text styles come from the scale.
pub(crate) fn apply_scale(style: &mut egui::Style) {
    let row = style.spacing.interact_size.y;
    omacraft_ui::scale::apply(style);
    style.spacing.interact_size.y = row;
}

/// Inter Tabular at a given size: row/column headers, which follow the sheet zoom.
pub fn numeric(size: f32) -> FontId {
    FontId::new(size, Face::Numeric.family())
}

/// A `RichText` in a role's face and size (dim roles still take their colour from the caller).
pub fn text(text: impl Into<String>, role: Role) -> egui::RichText {
    egui::RichText::new(text).font(role.font())
}

/// Keep only system faces egui can open. epaint panics in `set_fonts` on a face it can't parse,
/// so a broken file costs a fallback, never a crash. Runs on the worker, never in a frame.
pub(crate) fn validated(mut faces: SystemFaces) -> SystemFaces {
    use skrifa::MetadataProvider;
    let ok = |f: &omacraft_ui::fonts::SystemFace| skrifa::FontRef::from_index(&f.data, f.index).is_ok_and(|font| font.charmap().has_map());
    faces.ui.retain(ok);
    faces.ui_bold.retain(ok);
    faces.mono = faces.mono.filter(ok);
    faces
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with(defs: FontDefinitions) -> egui::Context {
        let ctx = egui::Context::default();
        ctx.set_fonts(defs);
        crate::theme::apply(&ctx, false);
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
        out.textures_delta.clear();
        ctx
    }

    fn width(ctx: &egui::Context, text: &str, font: FontId) -> f32 {
        ctx.fonts_mut(|f| f.layout_no_wrap(text.into(), font, egui::Color32::WHITE).size().x)
    }

    /// The owner's alpha 3 report: on Omarchy every chrome surface was monospaced. Chrome text
    /// styles and families must lead with bundled Inter, never a mono face, whatever mono the
    /// system has.
    #[test]
    fn chrome_text_styles_resolve_to_inter_not_mono() {
        let _guard = crate::fonts::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let defs = crate::theme::pinned_font_definitions();
        for family in [FontFamily::Proportional, FontFamily::Name(UI.into())] {
            assert_eq!(defs.families[&family][0], omacraft_ui::fonts::keys::INTER, "{family:?}");
        }
        assert_eq!(defs.families[&FontFamily::Name(UI_BOLD.into())][0], omacraft_ui::fonts::keys::INTER_STRONG);
        assert_eq!(defs.families[&Face::Numeric.family()][0], omacraft_ui::fonts::keys::NUMERIC);

        let ctx = ctx_with(defs);
        let style = ctx.global_style();
        for ts in [egui::TextStyle::Body, egui::TextStyle::Button, egui::TextStyle::Small, egui::TextStyle::Heading] {
            let font = &style.text_styles[&ts];
            assert!(matches!(font.family, FontFamily::Proportional | FontFamily::Name(_)), "{ts:?}");
            assert_ne!(font.family, FontFamily::Monospace, "{ts:?}");
            // Proportional: an `i` is far narrower than an `M`.
            assert!(width(&ctx, "iiii", font.clone()) < width(&ctx, "MMMM", font.clone()) * 0.6, "{ts:?} is monospaced");
        }
        for role in [Role::Menu, Role::Shortcut, Role::DialogTitle, Role::Body, Role::Section, Role::Caption] {
            assert_eq!(style.text_styles.get(&role.text_style()), Some(&role.font()), "{role:?} is registered");
            assert!(width(&ctx, "iiii", role.font()) < width(&ctx, "MMMM", role.font()) * 0.6, "{role:?} is monospaced");
        }
        // The mono role stays mono, and only it.
        assert_eq!(style.text_styles[&egui::TextStyle::Monospace].family, FontFamily::Monospace);
        assert!((width(&ctx, "iiii", Role::Mono.font()) - width(&ctx, "MMMM", Role::Mono.font())).abs() < 0.01);
        // Header digits line up.
        let digits: Vec<f32> = "0123456789".chars().map(|c| width(&ctx, &c.to_string(), numeric(11.0))).collect();
        assert!(digits.windows(2).all(|w| (w[0] - w[1]).abs() < 0.01), "{digits:?}");
    }

    /// Document families keep upstream's chains: Inter is chrome only.
    #[test]
    fn document_families_are_not_moved_to_inter() {
        let _guard = crate::fonts::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let defs = crate::theme::pinned_font_definitions();
        for family in [crate::theme::CELL, crate::theme::MONO, crate::theme::SERIF, crate::theme::DOCUMENT_UI, crate::theme::DOCUMENT_UI_BOLD] {
            let chain = &defs.families[&FontFamily::Name(family.into())];
            assert!(!chain.iter().any(|k| k.starts_with("omacraft/")), "{family}: {chain:?}");
        }
    }

    #[test]
    fn install_is_idempotent_and_the_switch_moves_only_ui_tokens() {
        let _guard = crate::fonts::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let base = crate::theme::pinned_font_definitions();
        let mut again = base.clone();
        install(&mut again, &ChromeFonts::default(), &[]);
        assert_eq!(again.families, base.families);
        // A broken "system" face is dropped before it can reach egui.
        let bogus = omacraft_ui::fonts::SystemFace {
            family: "Broken Sans".into(),
            path: "/nonexistent/broken.ttf".into(),
            index: 0,
            data: std::sync::Arc::new(vec![0; 64]),
        };
        let faces = validated(SystemFaces { ui: vec![bogus.clone()], ui_bold: vec![], mono: Some(bogus) });
        assert!(faces.ui.is_empty() && faces.mono.is_none());
        let mut system = base.clone();
        install(&mut system, &ChromeFonts { options: FontOptions { use_system_font: true }, system: faces }, &[]);
        assert_eq!(system.families[&FontFamily::Name(UI.into())][0], omacraft_ui::fonts::keys::INTER, "no system face: Inter still leads");
        assert_eq!(system.families[&Face::Display.family()][0], omacraft_ui::fonts::keys::DISPLAY);
    }
}
