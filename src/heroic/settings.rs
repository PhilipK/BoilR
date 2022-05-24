use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HeroicSettings {
    pub enabled: bool,
    pub launch_games_through_heroic: Vec<String>,
    pub default_launch_through_heroic: bool,
}
