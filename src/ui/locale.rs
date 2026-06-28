#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLanguage {
    English,
    PortugueseBR,
}

impl Default for AppLanguage {
    fn default() -> Self {
        AppLanguage::English
    }
}

impl AppLanguage {
    pub fn from_string(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "pt" | "pt-br" | "pt_br" | "pt-brasil" | "pt_brasil" => AppLanguage::PortugueseBR,
            _ => AppLanguage::English,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            AppLanguage::English => "en",
            AppLanguage::PortugueseBR => "pt-BR",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            AppLanguage::English => "English",
            AppLanguage::PortugueseBR => "Português (BR)",
        }
    }

    pub fn t(&self, key: &'static str) -> &'static str {
        match self {
            AppLanguage::English => english(key),
            AppLanguage::PortugueseBR => portuguese_br(key),
        }
    }
}

fn english(key: &'static str) -> &'static str {
    match key {
        "settings" => "Settings",
        "language" => "Language",
        "starting_import" => "Starting Import",
        "found_games_to_import" => "Found {games_found} games to import",
        "searching_for_images" => "Searching for images",
        "downloading_images" => "Downloading {to_download} images",
        "done_importing_games" => "Done importing games",
        "error" => "Error",
        "import_button_tooltip" => "Import your games into steam",
        "waiting_for_sync" => "Waiting for sync to finish",
        "import_games" => "Import Games",
        "images" => "Images",
        "backup" => "Backup",
        "disconnect" => "Disconnect",
        "save_settings" => "Save settings",
        "import_your_games" => "Import your games into steam",
        "select_games" => "Select the games you want to import into steam",
        "need_to_find_games" => "Need to find games",
        "finding_installed_games" => "Finding installed games",
        "did_not_find_any_games" => "Did not find any games",
        "rename" => "Rename",
        "select_all" => "Select all",
        "clear_selection" => "Clear selection",
        "selected" => "Selected",
        "games" => "games",
        "failed_finding_games" => "Failed finding games",
        "check_settings" => "Check the settings if BoilR didn't find the game you where looking for",
        "steam" => "Steam",
        "steam_location" => "Steam Location:",
        "create_collections" => "Create collections",
        "optimize_big_picture" => "Optimize for big picture",
        "stop_steam_before_import" => "Stop Steam before import",
        "start_steam_after_import" => "Start Steam after import",
        "steamgriddb" => "SteamGridDB",
        "download_images" => "Download images",
        "authentication_key" => "Authentication key:",
        "paste_from_clipboard" => "Paste from clipboard",
        "prefer_animated_images" => "Prefer animated images",
        "only_download_boilr_shortcuts" => "Only download images for BoilR shortcuts",
        "allow_nsfw" => "Allow NSFW images",
        "backups" => "Backups",
        "restore_backup" => "Here you can restore backed up shortcuts files",
        "click_backup_to_restore" => "Click a backup to restore it, your current shortcuts will be backed up first",
        "create_new_backup" => "Click here to create a new backup",
        "no_backups_found" => "No backups found, they will be created every time you run import",
        "not_selected" => "Not selected",
        "failed_to_find_games" => "Failed to find games",
        _ => key,
    }
}

fn portuguese_br(key: &'static str) -> &'static str {
    match key {
        "settings" => "Configurações",
        "language" => "Idioma",
        "starting_import" => "Iniciando importação",
        "found_games_to_import" => "Foram encontrados {games_found} jogos para importar",
        "searching_for_images" => "Procurando imagens",
        "downloading_images" => "Baixando {to_download} imagens",
        "done_importing_games" => "Importação concluída",
        "error" => "Erro",
        "import_button_tooltip" => "Importe seus jogos para o Steam",
        "waiting_for_sync" => "Aguardando a sincronização terminar",
        "import_games" => "Importar jogos",
        "images" => "Imagens",
        "backup" => "Backup",
        "disconnect" => "Desconectar",
        "save_settings" => "Salvar configurações",
        "import_your_games" => "Importe seus jogos para o Steam",
        "select_games" => "Selecione os jogos que você quer importar para o Steam",
        "need_to_find_games" => "Precisa encontrar jogos",
        "finding_installed_games" => "Procurando jogos instalados",
        "did_not_find_any_games" => "Nenhum jogo encontrado",
        "rename" => "Renomear",
        "select_all" => "Selecionar tudo",
        "clear_selection" => "Limpar seleção",
        "selected" => "Selecionados",
        "games" => "jogos",
        "failed_finding_games" => "Falha ao encontrar jogos",
        "check_settings" => "Verifique as configurações se o BoilR não encontrou o jogo que você procurava",
        "steam" => "Steam",
        "steam_location" => "Local do Steam:",
        "create_collections" => "Criar coleções",
        "optimize_big_picture" => "Otimizar para Big Picture",
        "stop_steam_before_import" => "Parar o Steam antes da importação",
        "start_steam_after_import" => "Iniciar o Steam depois da importação",
        "steamgriddb" => "SteamGridDB",
        "download_images" => "Baixar imagens",
        "authentication_key" => "Chave de autenticação:",
        "paste_from_clipboard" => "Colar da área de transferência",
        "prefer_animated_images" => "Preferir imagens animadas",
        "only_download_boilr_shortcuts" => "Baixar imagens apenas para atalhos do BoilR",
        "allow_nsfw" => "Permitir imagens NSFW",
        "backups" => "Backups",
        "restore_backup" => "Aqui você pode restaurar arquivos de atalhos salvos",
        "click_backup_to_restore" => "Clique em um backup para restaurá-lo; seus atalhos atuais serão salvos primeiro",
        "create_new_backup" => "Clique aqui para criar um novo backup",
        "no_backups_found" => "Nenhum backup encontrado; eles serão criados sempre que você executar a importação",
        "not_selected" => "Não selecionados",
        "failed_to_find_games" => "Falha ao encontrar jogos",
        _ => key,
    }
}
