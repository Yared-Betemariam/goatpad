use super::{DARK_THEME_ID, LIGHT_THEME_ID, Theme};
use crate::services::{paths::AppPaths, persistence::atomic_write};
use std::{fs, io, path::PathBuf};

pub fn ensure_default_themes(paths: &AppPaths) -> io::Result<()> {
    for theme in [Theme::default_dark(), Theme::default_light()] {
        let path = theme_path(paths, &theme.name);
        if !path.exists() {
            save_theme(paths, &theme)?;
        }
    }
    Ok(())
}

pub fn load_themes(paths: &AppPaths) -> io::Result<Vec<Theme>> {
    let mut themes = fs::read_dir(paths.themes_dir())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .filter_map(|path| fs::read(path).ok())
        .filter_map(|contents| serde_json::from_slice::<Theme>(&contents).ok())
        .collect::<Vec<_>>();
    themes.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(themes)
}

pub fn save_theme(paths: &AppPaths, theme: &Theme) -> io::Result<()> {
    let data = serde_json::to_vec_pretty(theme)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    atomic_write(&theme_path(paths, &theme.name), &data)
}

pub fn delete_theme(paths: &AppPaths, name: &str) -> io::Result<bool> {
    if name == DARK_THEME_ID || name == LIGHT_THEME_ID {
        return Ok(false);
    }
    let path = theme_path(paths, name);
    if path.exists() {
        fs::remove_file(path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn theme_path(paths: &AppPaths, name: &str) -> PathBuf {
    let slug = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned();
    paths.themes_dir().join(format!(
        "{}.json",
        if slug.is_empty() { "custom" } else { &slug }
    ))
}

#[cfg(test)]
mod tests {
    use super::{delete_theme, ensure_default_themes, load_themes, save_theme};
    use crate::{
        appearance::{Theme, ThemeColor},
        services::paths::AppPaths,
    };
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn saved_custom_theme_is_loaded_with_default_presets() {
        let directory = PathBuf::from(std::env::temp_dir())
            .join(format!("goatpad-theme-test-{}", Uuid::new_v4()));
        let paths = AppPaths::for_test(directory).unwrap();
        ensure_default_themes(&paths).unwrap();
        let mut custom = Theme::default_dark();
        custom.name = "My writing theme".to_owned();
        custom.primary = ThemeColor::rgb(10, 20, 30);
        custom.system_font = "Segoe UI".to_owned();
        custom.content_font = "Georgia".to_owned();
        save_theme(&paths, &custom).unwrap();

        let themes = load_themes(&paths).unwrap();
        assert!(themes.iter().any(|theme| theme == &custom));
        assert!(themes.iter().any(|theme| theme.name == "default-dark"));
        assert!(themes.iter().any(|theme| theme.name == "default-light"));

        // Test delete_theme
        assert!(delete_theme(&paths, "My writing theme").unwrap());
        let themes_after = load_themes(&paths).unwrap();
        assert!(
            !themes_after
                .iter()
                .any(|theme| theme.name == "My writing theme")
        );

        // Default themes cannot be deleted
        assert!(!delete_theme(&paths, "default-dark").unwrap());
    }
}
