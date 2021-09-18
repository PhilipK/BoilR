use super::legendary_game::LegendaryGame;
use serde_json::from_str;
use std::process::Command;
use std::{error::Error};

pub fn get_legendary_games() -> Result<Vec<LegendaryGame>, Box<dyn Error>> {
    let legendary_command = Command::new("legendary")
        .arg("list-installed")
        .arg("--json")
        .output()?;
    let json = String::from_utf8_lossy(&legendary_command.stdout);
    let legendary_ouput = from_str(&json)?;
    Ok(legendary_ouput)
}