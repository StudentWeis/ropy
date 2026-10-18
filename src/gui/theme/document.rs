//! Editable theme data, independent of window controls and file storage.
use super::{ThemeDefinition, ThemeId, ThemeMode, ThemePalette};

#[derive(Clone, Debug, PartialEq, Eq)]
#[expect(clippy::redundant_pub_crate)]
pub(crate) struct ThemeDocument {
    pub(crate) name: String,
    pub(crate) mode: ThemeMode,
    pub(crate) palette: ThemePalette,
}

impl From<&ThemeDefinition> for ThemeDocument {
    fn from(theme: &ThemeDefinition) -> Self {
        Self {
            name: theme.name.clone(),
            mode: theme.mode,
            palette: theme.palette,
        }
    }
}

impl ThemeDocument {
    pub(crate) fn definition(&self, id: ThemeId) -> ThemeDefinition {
        ThemeDefinition {
            id,
            name: self.name.trim().to_owned(),
            mode: self.mode,
            palette: self.palette,
        }
    }

    pub(crate) fn serialize(&self) -> Result<String, DocumentError> {
        if self.name.trim().is_empty() {
            return Err(DocumentError::EmptyName);
        }
        let mut table = toml::Table::new();
        table.insert("theme_name".into(), self.name.trim().into());
        table.insert(
            "mode".into(),
            match self.mode {
                ThemeMode::Light => "light",
                ThemeMode::Dark => "dark",
            }
            .into(),
        );
        for role in ColorRole::ALL {
            table.insert(
                role.field().into(),
                format!("#{:06x}", role.get(&self.palette)).into(),
            );
        }
        Ok(toml::to_string_pretty(&table)?)
    }
}

#[derive(Debug, thiserror::Error)]
#[expect(clippy::redundant_pub_crate)]
pub(crate) enum DocumentError {
    #[error("theme name must not be empty")]
    EmptyName,
    #[error(transparent)]
    Serialize(#[from] toml::ser::Error),
}

// One mapping owns editor order, names and typed palette access.
macro_rules! color_roles {
    ($($role:ident => $field:ident, label_key: $label:literal;)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[expect(clippy::redundant_pub_crate)]
        pub(crate) enum ColorRole { $($role,)* }
        impl ColorRole {
            pub(crate) const ALL: &'static [Self] = &[$(Self::$role,)*];
            pub(crate) const fn field(self) -> &'static str { match self { $(Self::$role => stringify!($field),)* } }
            pub(crate) const fn label_key(self) -> &'static str { match self { $(Self::$role => $label,)* } }
            pub(crate) const fn get(self, palette: &ThemePalette) -> u32 { match self { $(Self::$role => palette.$field,)* } }
            pub(crate) const fn set(self, palette: &mut ThemePalette, value: u32) { match self { $(Self::$role => palette.$field = value,)* } }
        }
    };
}

color_roles! {
    Background => background, label_key: "theme_color_background";
    Foreground => foreground, label_key: "theme_color_foreground";
    Primary => primary, label_key: "theme_color_primary";
    PrimaryForeground => primary_foreground, label_key: "theme_color_primary_foreground";
    Secondary => secondary, label_key: "theme_color_secondary";
    Border => border, label_key: "theme_color_border";
    Accent => accent, label_key: "theme_color_accent";
    AccentForeground => accent_foreground, label_key: "theme_color_accent_foreground";
    SecondaryForeground => secondary_foreground, label_key: "theme_color_secondary_foreground";
    Muted => muted, label_key: "theme_color_muted";
    MutedForeground => muted_foreground, label_key: "theme_color_muted_foreground";
    Input => input, label_key: "theme_color_input";
    PrimaryHover => primary_hover, label_key: "theme_color_primary_hover";
    PrimaryActive => primary_active, label_key: "theme_color_primary_active";
    Danger => danger, label_key: "theme_color_danger";
    DangerForeground => danger_foreground, label_key: "theme_color_danger_foreground";
    Popover => popover, label_key: "theme_color_popover";
    PopoverForeground => popover_foreground, label_key: "theme_color_popover_foreground";
    Selection => selection, label_key: "theme_color_selection";
    Ring => ring, label_key: "theme_color_ring";
    ListHover => list_hover, label_key: "theme_color_list_hover";
    ListActive => list_active, label_key: "theme_color_list_active";
    ScrollbarThumb => scrollbar_thumb, label_key: "theme_color_scrollbar_thumb";
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn test_document_roundtrip_preserves_every_bundled_palette() {
        for id in super::super::available_themes() {
            let theme = ThemeDefinition::load(&id).unwrap();
            let document = ThemeDocument::from(&theme);
            let parsed = ThemeDefinition::parse(&id, &document.serialize().unwrap()).unwrap();
            assert_eq!(parsed, theme);
        }
    }
}
