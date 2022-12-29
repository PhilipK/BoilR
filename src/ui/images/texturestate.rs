#[derive(Clone)]
pub enum TextureDownloadState {
    Downloading,
    Downloaded,
    Loaded(egui::TextureHandle),
    Failed,
}
