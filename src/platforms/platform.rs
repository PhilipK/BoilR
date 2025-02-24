use dyn_clone::DynClone;
use steam_shortcuts_util::shortcut::ShortcutOwned;

pub trait GamesPlatform
where
    Self: std::marker::Send,
    Self: std::marker::Sync,
    Self: DynClone,
{
    fn name(&self) -> &str;

    fn code_name(&self) -> &str;

    fn enabled(&self) -> bool;

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>>;

    fn get_settings_serializable(&self) -> String;

    fn render_ui(&mut self, ui: &mut egui::Ui);
}

dyn_clone::clone_trait_object!(GamesPlatform);

#[derive(Clone)]
pub struct ShortcutToImport {
    pub shortcut: ShortcutOwned,
    pub needs_proton: bool,
    pub needs_symlinks: bool,
}

pub(crate) fn to_shortcuts<T, P>(
    platform: &P,
    into_shortcuts: Result<Vec<T>, eyre::ErrReport>,
) -> eyre::Result<Vec<ShortcutToImport>>
where
    T: Into<ShortcutOwned>,
    T: NeedsProton<P>,
{
    let shortcuts = into_shortcuts?;
    let mut shortcut_info = vec![];
    for m in shortcuts {
        let needs_proton = m.needs_proton(platform);
        let needs_symlinks = m.create_symlinks(platform);
        let shortcut = m.into();
        shortcut_info.push(ShortcutToImport {
            shortcut,
            needs_proton,
            needs_symlinks,
        });
    }
    Ok(shortcut_info)
}

pub(crate) fn to_shortcuts_simple<T>(
    into_shortcuts: Result<Vec<T>, eyre::ErrReport>,
) -> eyre::Result<Vec<ShortcutToImport>>
where
    T: Into<ShortcutOwned>,
{
    let shortcuts = into_shortcuts?;
    let mut shortcut_info = vec![];
    for m in shortcuts {
        let needs_proton = false;
        let needs_symlinks = false;
        let shortcut = m.into();
        shortcut_info.push(ShortcutToImport {
            shortcut,
            needs_proton,
            needs_symlinks,
        });
    }
    Ok(shortcut_info)
}

pub trait NeedsProton<P> {
    fn needs_proton(&self, platform: &P) -> bool;

    fn create_symlinks(&self, platform: &P) -> bool;
}
