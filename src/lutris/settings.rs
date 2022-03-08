use serde::{Serialize,Deserialize};

#[derive(Debug, Deserialize, Serialize,Clone)]
pub struct LutrisSettings {
    pub enabled: bool,
    pub executable: Option<String>,
}