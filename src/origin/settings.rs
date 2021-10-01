use serde::Deserialize;

#[derive(Debug, Deserialize,Clone)]

pub struct OriginSettings {
    pub enabled: bool,
    pub path: Option<String>,
}
