use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct Receipt {
    pub game: Game,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct Game {
    pub title: String,
    /// butler writes the whole game object; older receipts may lack the id.
    #[serde(default)]
    pub id: Option<u64>,
}
