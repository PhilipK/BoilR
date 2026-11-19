use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

use serde::Deserialize;

use super::LutrisSettings;

#[derive(Deserialize, Clone)]
pub struct LutrisGame {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub service: Option<String>,
    /// The service's own record of the game, as a JSON text. Lutris only lists it for the
    /// games of a connected store account.
    #[serde(default)]
    pub details: Option<String>,
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
        let mut shortcut = Shortcut::new(
            "0",
            game.name.as_str(),
            exectuable.as_str(),
            "",
            "",
            "",
            options.as_str(),
        )
        .to_owned();
        if let Some(identity) = game.identity() {
            shortcut.dev_kit_game_id = identity;
        }
        shortcut
    }
}

impl LutrisGame {
    /// The game's identity in its store, when Lutris lists the game from a connected store
    /// account and says which game it is there. A store account in BoilR that owns the
    /// same game then does not add it a second time.
    pub fn identity(&self) -> Option<String> {
        let details: serde_json::Value = serde_json::from_str(self.details.as_deref()?).ok()?;
        let text = |value: &serde_json::Value| match value {
            serde_json::Value::String(text) if !text.is_empty() => Some(text.clone()),
            serde_json::Value::Number(number) => Some(number.to_string()),
            _ => None,
        };
        let (store, id) = match self.service.as_deref()? {
            // GOG's product id.
            "gog" => ("gog", details.get("id").and_then(text)?),
            // Epic's `appName`.
            "egs" => ("epic", details.get("appName").and_then(text)?),
            _ => return None,
        };
        Some(crate::sync::shortcut_identity(store, &id))
    }

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
