//! The desktop's interface font, found among the installed fonts in the
//! three weights that the window uses.

use crate::portal::appearance::DesktopFont;
use ab_glyph::{Font, FontRef};
use egui::epaint::text::{FontTweak, VariationCoords};
use egui::FontData;
use fontique::{
    Collection, CollectionOptions, FamilyInfo, FontStyle, FontWeight, FontWidth, SourceCache,
};
use std::sync::Arc;
use tracing::debug;

/// The regular, medium, and semibold faces of the desktop font.
#[derive(Clone)]
pub struct Faces {
    pub family: String,
    pub regular: Arc<FontData>,
    pub medium: Arc<FontData>,
    pub semibold: Arc<FontData>,
    /// The line of the regular face, from its ascent to its descent with
    /// its line gap, in em
    pub line: f32,
}

impl std::fmt::Debug for Faces {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Faces")
            .field("family", &self.family)
            .field("line", &self.line)
            .finish_non_exhaustive()
    }
}

/// `None` when no installed font has the family, or its file does not
/// read. Enumerating the fonts can take a few hundred milliseconds.
pub fn load(font: &DesktopFont) -> Option<Faces> {
    let mut collection = Collection::new(CollectionOptions {
        shared: false,
        system_fonts: true,
    });
    let mut cache = SourceCache::default();
    let Some(family) = collection.family_by_name(&font.family) else {
        debug!("No installed font has the desktop's family {}", font.family);
        return None;
    };
    let regular = face(&family, FontWeight::NORMAL, &mut cache)?;
    let line = line_of(&regular)?;
    let medium = face(&family, FontWeight::MEDIUM, &mut cache).unwrap_or_else(|| regular.clone());
    let semibold =
        face(&family, FontWeight::SEMI_BOLD, &mut cache).unwrap_or_else(|| medium.clone());
    Some(Faces {
        family: font.family.clone(),
        regular: Arc::new(regular),
        medium: Arc::new(medium),
        semibold: Arc::new(semibold),
        line,
    })
}

/// The face nearest `weight`. A variable font gets the coordinates of that
/// weight, since one file holds every weight.
fn face(family: &FamilyInfo, weight: FontWeight, cache: &mut SourceCache) -> Option<FontData> {
    let info = family.match_font(FontWidth::NORMAL, FontStyle::Normal, weight, false)?;
    let blob = info.load(Some(cache))?;
    let mut data = FontData::from_owned(blob.data().to_vec());
    data.index = info.index();
    let synthesis = info.synthesis(FontWidth::NORMAL, FontStyle::Normal, weight);
    let settings = synthesis.variation_settings();
    if !settings.is_empty() {
        let coords = VariationCoords::new(
            settings
                .iter()
                .map(|(tag, value)| (u32::from_be_bytes(tag.to_be_bytes()), *value)),
        );
        data = data.tweak(FontTweak {
            coords,
            ..FontTweak::default()
        });
    }
    Some(data)
}

fn line_of(data: &FontData) -> Option<f32> {
    let bytes: &[u8] = (*data.font).as_ref();
    let font = FontRef::try_from_slice_and_index(bytes, data.index).ok()?;
    let em = font.units_per_em()?;
    let line = (font.height_unscaled() + font.line_gap_unscaled()) / em;
    (line.is_finite() && line > 0.5).then_some(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geist_has_the_line_that_the_type_scale_expects() {
        let geist = FontData::from_static(crate::fonts::GEIST_REGULAR);
        let line = line_of(&geist).unwrap();
        assert!((line - 1.3).abs() < 0.01, "{line}");
    }

    #[test]
    fn a_family_that_no_font_has_gives_none() {
        assert!(load(&DesktopFont {
            family: "No Such Family 7f3a".into(),
            size: 10.0,
        })
        .is_none());
    }
}
