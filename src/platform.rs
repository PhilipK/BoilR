use steam_shortcuts_util::shortcut::ShortcutOwned;

pub trait Platform<T, E>
where
    T: Into<ShortcutOwned>,
{
    fn enabled(&self) -> bool;

    fn name(&self) -> &str;

    fn get_shortcuts(&self) -> Result<Vec<T>, E>;

    #[cfg(target_os = "linux")]
    fn create_symlinks(&self) -> bool;
}
