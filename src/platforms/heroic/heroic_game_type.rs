use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

use super::{HeroicGame, InstallationMode};

#[derive(Clone)]
pub enum HeroicGameType {
    Epic(HeroicGame),
    //The bool is if it is windows (true) or not (false)
    Gog(crate::platforms::GogShortcut, bool),
    //The string is the app name
    Heroic {
        title: String,
        app_name: String,
        install_mode: InstallationMode,
    },
}

impl HeroicGameType {
    pub fn app_name(&self) -> &str {
        match self {
            HeroicGameType::Epic(g) => g.app_name.as_ref(),
            HeroicGameType::Gog(g, _) => g.game_id.as_ref(),
            HeroicGameType::Heroic {
                title: _,
                app_name,
                install_mode: _,
            } => app_name,
        }
    }

    pub(crate) fn title(&self) -> &str {
        match self {
            HeroicGameType::Epic(g) => g.title.as_ref(),
            HeroicGameType::Gog(g, _) => g.name.as_ref(),
            HeroicGameType::Heroic {
                title,
                app_name: _,
                install_mode: _,
            } => title.as_ref(),
        }
    }
}

impl From<HeroicGameType> for ShortcutOwned {
    fn from(heroic_game_type: HeroicGameType) -> Self {
        match heroic_game_type {
            HeroicGameType::Epic(epic) => epic.into(),
            HeroicGameType::Gog(gog, _) => gog.into(),
            HeroicGameType::Heroic {
                title,
                app_name,
                install_mode,
            } => {
                let launch_parameter = format!("heroic://launch/{app_name}");
                let (exe, parameter) = match install_mode {
                    InstallationMode::FlatPak => (
                        "flatpak",
                        format!(
                            "run com.heroicgameslauncher.hgl {launch_parameter} --no-gui --no-sandbox"
                        ),
                    ),
                    InstallationMode::UserBin => ("heroic", launch_parameter),
                };
                Shortcut::new("0", title.as_str(), exe, "", "", "", parameter.as_str()).to_owned()
            }
        }
    }
}
