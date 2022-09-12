use std::{error::Error, process::Command};

use eframe::epaint::ahash::HashMap;
use serde::{Deserialize, Serialize};

use crate::platform::Platform;

use super::BottlesSettings;
use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BottlesPlatform {
    pub settings: BottlesSettings,
}

#[derive(Debug, Clone)]
pub struct BottlesApp {
    pub name: String,
    pub bottle: String,
}

impl From<BottlesApp> for ShortcutOwned {
    fn from(app: BottlesApp) -> Self {
        //
        let launch_parameter = format!(
            "run --command=bottles-cli com.usebottles.bottles run -b \"{}\" -p \"{}\"",
            app.bottle, app.name
        );
        Shortcut::new("0", &app.name, "flatpak", "", "", "", &launch_parameter).to_owned()
    }
}

impl Platform<BottlesApp, Box<dyn Error>> for BottlesPlatform {
    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn name(&self) -> &str {
        "Bottles"
    }

    fn get_shortcuts(&self) -> Result<Vec<BottlesApp>, Box<dyn Error>> {
        let mut res = vec![];
        let bottles = get_bottles();
        dbg!(&bottles);
        for bottle in bottles {
            for (_id,program) in bottle.external_programs {
                res.push(BottlesApp{
                    name:program.name,
                    bottle: bottle.name.clone()
                })
            }
        }
        Ok(res)
    }

    fn settings_valid(&self) -> crate::platform::SettingsValidity {
        if let Ok(s) =self.get_shortcuts(){
            if !s.is_empty() {
                return crate::platform::SettingsValidity::Valid;
            }
        }
        crate::platform::SettingsValidity::Invalid { reason: String::from("Nothing found")}
    }

    fn create_symlinks(&self) -> bool {
        false
    }

    fn needs_proton(&self, _input: &BottlesApp) -> bool {
        false
    }
}

fn get_bottles() -> Vec<Bottle> {
    let output_result = get_bottles_output();
    match output_result {
        Ok(json) => {
            let map : HashMap<String,Bottle>= serde_json::from_str(json.as_str()).unwrap_or_default();
            let mut res = vec![];
            for (_,value) in map{
                res.push(value);
            }
            res
        },
        Err(_err) => vec![],
    }
}

fn get_bottles_output() -> Result<String, Box<dyn Error>> {
    let output = {
        #[cfg(not(feature = "flatpak"))]
        {
            let mut command = Command::new("flatpak");
            command
                .arg("run")
                .arg("--command=bottles-cli")
                .arg("com.usebottles.bottles")
                .arg("-j")
                .arg("list")
                .arg("bottles")
                .output()?
        }
        #[cfg(feature = "flatpak")]
        {
            let mut command = Command::new("flatpak-spawn");
            command
                .arg("--host")
                .arg("flatpak")
                .arg("run")
                .arg("--command=bottles-cli")
                .arg("com.usebottles.bottles")
                .arg("-j")
                .arg("list")
                .arg("bottles")
                .output()?
        }
    };
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[derive(Deserialize,Debug)]
struct Bottle {
    #[serde(alias = "Name")]
    pub(crate) name: String,
    #[serde(alias = "External_Programs")]
    pub(crate) external_programs : HashMap<String,Program>,
}

#[derive(Deserialize,Debug)]
struct Program {
    #[serde(alias = "Name")]
    pub(crate) name: String,
}

