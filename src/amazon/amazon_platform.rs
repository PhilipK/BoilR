use std::error::Error;

use crate::platform::Platform;

use super::{AmazonSettings, AmazonGame};


pub struct AmazonPlatform{
    pub settings:AmazonSettings
}

impl Platform<AmazonGame, Box<dyn Error>> for AmazonPlatform{
    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn name(&self) -> &str {
        "Amazon"
    }

    fn get_shortcuts(&self) -> Result<Vec<AmazonGame>, Box<dyn Error>> {
        todo!()
    }

    fn settings_valid(&self) -> crate::platform::SettingsValidity {
        todo!()
    }

    fn create_symlinks(&self) -> bool {
        false
    }

    fn needs_proton(&self, _input: &AmazonGame) -> bool {
        false
    }
}


