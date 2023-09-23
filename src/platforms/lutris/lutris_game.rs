use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

use serde::Deserialize;

use super::LutrisSettings;

#[derive(Deserialize, Clone)]
pub struct LutrisGame {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub service: Option<String>,
    pub runner: Option<String>,
    // pub platform: String,
    // pub installed: bool,
    // pub details: String,
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
        let run_game = self
            .settings
            .as_ref()
            .map(|s| s.installed)
            .map(|i| if i { ":rungame/" } else { ":" })
            .unwrap_or_default();

        if is_flatpak {
            format!(
                "run {} lutris{}{}",
                self.settings
                    .as_ref()
                    .map(|s| s.flatpak_image.clone())
                    .unwrap_or_default(),
                run_game,
                self.slug
            )
        } else {
            format!("lutris{}{}", run_game, self.slug)
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
