use super::*;

impl GoatpadApp {
    pub(in crate::app) fn select_theme(&mut self, ctx: &egui::Context, theme: Theme) {
        self.settings.theme = theme.name.clone();
        self.theme_draft = theme;
        self.apply_theme(ctx, &self.theme_draft.clone());
        if let Err(error) = self.settings.save(&self.paths) {
            self.report_error(format!("Could not save the active theme: {error}"));
        }
    }

    pub(in crate::app) fn apply_theme(&mut self, ctx: &egui::Context, theme: &Theme) {
        apply_theme(ctx, theme);
        self.title_bar_color = theme.title_bar_color();
    }

    pub(in crate::app) fn start_create_theme(&mut self) {
        let mut new_theme = self.theme_draft.clone();
        new_theme.name = "Custom Theme".to_owned();
        self.editing_theme = Some(new_theme);
        self.editing_theme_is_new = true;
    }

    pub(in crate::app) fn duplicate_theme(&mut self, base: &Theme) {
        let mut clone = base.clone();
        clone.name = format!("{} Copy", base.name);
        self.editing_theme = Some(clone);
        self.editing_theme_is_new = true;
    }

    pub(in crate::app) fn save_editing_theme(&mut self, ctx: &egui::Context) {
        let Some(mut theme) = self.editing_theme.take() else {
            return;
        };
        let trimmed_name = theme.name.trim().to_owned();
        if trimmed_name.is_empty() {
            self.report_error("Theme name cannot be empty");
            self.editing_theme = Some(theme);
            return;
        }
        theme.name = trimmed_name;

        if let Err(error) = save_theme(&self.paths, &theme) {
            self.report_error(format!("Could not save theme: {error}"));
            self.editing_theme = Some(theme);
            return;
        }

        if let Some(existing) = self.themes.iter_mut().find(|t| t.name == theme.name) {
            *existing = theme.clone();
        } else {
            self.themes.push(theme.clone());
            self.themes.sort_by(|a, b| a.name.cmp(&b.name));
        }

        if self.editing_theme_is_new || self.settings.theme == theme.name {
            self.select_theme(ctx, theme);
        }
        self.editing_theme_is_new = false;
        self.report_success("Theme saved");
    }

    pub(in crate::app) fn delete_custom_theme(&mut self, ctx: &egui::Context, theme_name: &str) {
        if self
            .themes
            .iter()
            .find(|theme| theme.name == theme_name)
            .is_some_and(Theme::is_builtin)
        {
            self.report_error("Built-in themes cannot be deleted");
            return;
        }
        match appearance::delete_theme(&self.paths, theme_name) {
            Ok(true) => {
                self.themes.retain(|t| t.name != theme_name);
                self.report_success(format!("Deleted theme \"{theme_name}\""));
                if self.settings.theme == theme_name {
                    let fallback = self
                        .themes
                        .iter()
                        .find(|t| t.name == LIGHT_THEME_ID)
                        .cloned()
                        .unwrap_or_else(Theme::default_light);
                    self.select_theme(ctx, fallback);
                }
            }
            Ok(false) => {
                self.report_error(format!("Theme \"{theme_name}\" not found"));
            }
            Err(error) => {
                self.report_error(format!("Could not delete theme: {error}"));
            }
        }
    }
}
