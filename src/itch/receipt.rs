use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct Receipt {
    pub game: Game,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct Game {
    pub title: String,
}
