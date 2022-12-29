use std::path::PathBuf;

use steamgriddb_api::images::MimeTypes;


#[derive(Clone, Debug)]
pub struct PossibleImage {
    pub thumbnail_path: PathBuf,
    pub thumbnail_url: String,
    pub mime: MimeTypes,
    pub full_url: String,
}