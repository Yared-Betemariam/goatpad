mod fonts;
mod model;
mod store;
mod style;

#[allow(unused_imports)]
pub use fonts::{FONT_OPTIONS, install_fonts};
#[allow(unused_imports)]
pub use model::{
    DARK_THEME_ID, DEFAULT_FONT_FAMILY, DEFAULT_FONT_SIZE, DEFAULT_THEME_ID, LIGHT_THEME_ID,
    MAX_FONT_SIZE, MIN_FONT_SIZE, Theme, ThemeColor,
};
pub use store::{delete_theme, ensure_default_themes, load_themes, save_theme};
pub use style::apply_theme;
