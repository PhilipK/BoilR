use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HeroicSettings {
    pub enabled: bool,
    pub launch_games_through_heroic: Vec<String>,
    pub default_launch_through_heroic: bool,
}

impl HeroicSettings {
    pub fn is_heroic_launch<S: AsRef<str>>(&self, app_name: S) -> bool {
        let contains = self
            .launch_games_through_heroic
            .contains(&app_name.as_ref().to_string());
        if self.default_launch_through_heroic {
            !contains
        } else {
            contains
        }
    }
}
