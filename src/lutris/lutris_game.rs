use crate::lutris::settings::LutrisSettings;
use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

#[derive(Clone)]
pub struct LutrisGame {
    pub index: String,
    pub name: String,
    pub id: String,
    pub platform: String,
    pub settings: Option<LutrisSettings>,
}

impl From<LutrisGame> for ShortcutOwned {
    fn from(game: LutrisGame) -> Self {
        let options = game.get_options();
        let exectuable = game.get_executable();
        Shortcut::new(
            "0",
            game.name.as_str(),
            exectuable.as_str(),
            "",
            "",
            "",
            options.as_str(),
        )
        .to_owned()
    }
}

impl LutrisGame {
    pub fn get_options(&self) -> String {
        let is_flatpak = self
            .settings
            .as_ref()
            .map(|s| s.flatpak)
            .unwrap_or_default();
        if is_flatpak {
            format!(
                "run {} lutris:rungameid/{}",
                self.settings
                    .as_ref()
                    .map(|s| s.flatpak_image.clone())
                    .unwrap_or_default(),
                self.index
            )
        } else {
            format!("lutris:rungame/{}", self.id)
        }
    }

    pub fn get_executable(&self) -> String {
        let is_flatpak = self
            .settings
            .as_ref()
            .map(|s| s.flatpak)
            .unwrap_or_default();
        if is_flatpak {
            "flatpak".to_string()
        } else {
            self.settings
                .as_ref()
                .map(|s| s.executable.clone())
                .unwrap_or_default()
        }
    }
}
