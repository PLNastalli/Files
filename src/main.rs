//! Windows-first native file manager powered by GPUI.
//! This is an early MVP, not yet a drop-in replacement for explorer.exe.

use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    time::Duration,
};

use files::{
    filesystem::{self, FileEntry, SortBy},
    operations, shortcuts,
};
use gpui_kit::component::{
    Theme, ThemeMode, TitleBar,
    input::{InputEvent, InputState},
};
use gpui_kit::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, KeyBinding, Subscription, Window,
    WindowBounds, WindowOptions, actions, px, size,
};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

mod ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Details,
    Tiles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RibbonPage {
    Home,
    View,
    Themes,
}

actions!(
    files,
    [
        Back,
        Forward,
        Up,
        Refresh,
        NewFolder,
        Copy,
        Paste,
        Delete,
        OpenSelected,
        ToggleHidden,
        NewTab,
        CloseTab,
        ToggleDetails,
        ToggleView,
        Cut,
        Rename,
        ExportCsv,
        ToggleRowNumbers,
        SelectAll,
        ClearSelection,
        ShowProperties,
        OpenInNewTab,
    ]
);

struct FilesApp {
    directory: PathBuf,
    entries: Vec<FileEntry>,
    history: Vec<PathBuf>,
    history_index: usize,
    tabs: Vec<PathBuf>,
    active_tab: usize,
    details_open: bool,
    view_mode: ViewMode,
    ribbon_page: RibbonPage,
    show_navigation: bool,
    show_row_numbers: bool,
    selected: Option<PathBuf>,
    selected_paths: Vec<PathBuf>,
    copied: Option<PathBuf>,
    clipboard_cut: bool,
    rename_target: Option<PathBuf>,
    rename_input: Entity<InputState>,
    transfer_in_progress: bool,
    transfer_task: Option<gpui_kit::Task<()>>,
    sort: SortBy,
    show_hidden: bool,
    confirm_trash: bool,
    status: String,
    focus: FocusHandle,
    search: Entity<InputState>,
    _search_events: Subscription,
    watcher: Option<RecommendedWatcher>,
    change_rx: Option<Receiver<notify::Result<Event>>>,
    shortcuts: Vec<(String, PathBuf)>,
    pinned: Vec<PathBuf>,
}

impl FilesApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let home = dirs::home_dir()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("C:\\"));
        let mut shortcuts = vec![("Início".to_string(), home.clone())];
        for (label, folder) in [
            ("Área de Trabalho", "Desktop"),
            ("Documentos", "Documents"),
            ("Downloads", "Downloads"),
            ("Imagens", "Pictures"),
            ("Vídeos", "Videos"),
            ("Músicas", "Music"),
        ] {
            let path = home.join(folder);
            if path.is_dir() {
                shortcuts.push((label.to_string(), path));
            }
        }
        let pinned = shortcuts::config_path()
            .and_then(|path| shortcuts::load(&path))
            .unwrap_or_default();
        for path in &pinned {
            if !shortcuts.iter().any(|(_, entry)| entry == path) {
                shortcuts.push((path.file_name().map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string()), path.clone()));
            }
        }
        #[cfg(windows)]
        for drive in b'A'..=b'Z' {
            let path = PathBuf::from(format!("{}:\\", char::from(drive)));
            if path.is_dir() {
                shortcuts.push((format!("Disco ({}:)", char::from(drive)), path));
            }
        }

        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Pesquisar nesta pasta"));
        let rename_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Novo nome do arquivo"));
        let search_events = cx.subscribe_in(&search, window, |_, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });

        let result_path = home.clone();
        let mut result = Self {
            directory: home.clone(),
            entries: Vec::new(),
            history: vec![home],
            history_index: 0,
            tabs: vec![result_path.clone()],
            active_tab: 0,
            details_open: false,
            view_mode: ViewMode::Details,
            ribbon_page: RibbonPage::Home,
            show_navigation: true,
            show_row_numbers: false,
            selected: None,
            selected_paths: Vec::new(),
            copied: None,
            clipboard_cut: false,
            rename_target: None,
            rename_input,
            transfer_in_progress: false,
            transfer_task: None,
            sort: SortBy::Name,
            show_hidden: false,
            confirm_trash: false,
            status: "Pronto".to_string(),
            focus,
            search,
            _search_events: search_events,
            watcher: None,
            change_rx: None,
            shortcuts,
            pinned,
        };
        result.reload(cx);
        result.watch_directory();
        result.start_watcher_poll(cx);
        result
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        match filesystem::read_directory(&self.directory, self.show_hidden, self.sort) {
            Ok(entries) => {
                self.selected_paths
                    .retain(|path| entries.iter().any(|entry| &entry.path == path));
                self.selected = self.selected_paths.last().cloned();
                self.status = format!("{} itens", entries.len());
                self.entries = entries;
            }
            Err(error) => {
                self.entries.clear();
                self.selected_paths.clear();
                self.selected = None;
                self.status = format!("Erro ao abrir pasta: {error}");
            }
        }
        cx.notify();
    }

    fn watch_directory(&mut self) {
        self.watcher = None;
        self.change_rx = None;

        let (sender, receiver) = mpsc::channel();
        if let Ok(mut watcher) = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        }) && watcher
            .watch(&self.directory, RecursiveMode::NonRecursive)
            .is_ok()
        {
            self.watcher = Some(watcher);
            self.change_rx = Some(receiver);
        }
    }

    fn start_watcher_poll(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(750))
                    .await;
                if view
                    .update(cx, |this, cx| {
                        let (changed, error) = this
                            .change_rx
                            .as_ref()
                            .map(|rx| {
                                rx.try_iter()
                                    .fold((false, None), |(changed, error), event| match event {
                                        Ok(_) => (true, error),
                                        Err(reason) => (changed, Some(reason)),
                                    })
                            })
                            .unwrap_or((false, None));
                        if changed {
                            this.reload(cx);
                        }
                        if let Some(error) = error {
                            this.report_error("Monitor de arquivos", error, cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    fn navigate(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !path.is_dir() {
            self.status = format!("Não é uma pasta: {}", path.display());
            cx.notify();
            return;
        }
        if path == self.directory {
            self.reload(cx);
            return;
        }
        self.history.truncate(self.history_index + 1);
        self.history.push(path.clone());
        self.history_index += 1;
        self.change_directory(path, cx);
    }

    fn change_directory(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            *tab = path.clone();
        }
        self.directory = path;
        self.selected_paths.clear();
        self.selected = None;
        self.confirm_trash = false;
        self.rename_target = None;
        self.reload(cx);
        self.watch_directory();
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        if self.history_index > 0 {
            self.history_index -= 1;
            let path = self.history[self.history_index].clone();
            self.change_directory(path, cx);
        }
    }

    fn forward(&mut self, cx: &mut Context<Self>) {
        if self.history_index + 1 < self.history.len() {
            self.history_index += 1;
            let path = self.history[self.history_index].clone();
            self.change_directory(path, cx);
        }
    }

    fn up(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = self.directory.parent() {
            self.navigate(parent.to_path_buf(), cx);
        }
    }

    fn new_tab(&mut self, cx: &mut Context<Self>) {
        self.tabs.push(self.directory.clone());
        self.active_tab = self.tabs.len() - 1;
        self.history = vec![self.directory.clone()];
        self.history_index = 0;
        cx.notify();
    }

    fn select_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index == self.active_tab || index >= self.tabs.len() {
            return;
        }
        self.active_tab = index;
        let path = self.tabs[index].clone();
        self.history = vec![path.clone()];
        self.history_index = 0;
        self.change_directory(path, cx);
    }

    fn close_tab(&mut self, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.remove(self.active_tab);
        self.active_tab = self.active_tab.min(self.tabs.len() - 1);
        let path = self.tabs[self.active_tab].clone();
        self.history = vec![path.clone()];
        self.history_index = 0;
        self.change_directory(path, cx);
    }

    fn toggle_details(&mut self, cx: &mut Context<Self>) {
        self.details_open = !self.details_open;
        cx.notify();
    }

    fn show_ribbon(&mut self, page: RibbonPage, cx: &mut Context<Self>) {
        self.ribbon_page = page;
        cx.notify();
    }

    fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        self.view_mode = mode;
        cx.notify();
    }

    fn toggle_navigation(&mut self, cx: &mut Context<Self>) {
        self.show_navigation = !self.show_navigation;
        cx.notify();
    }

    /// Pin a real folder to quick access; changes persist across app restarts.
    fn pin_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !path.is_dir() {
            self.status = "Apenas pastas podem ser fixadas".to_owned();
            cx.notify();
            return;
        }
        if !self.pinned.contains(&path) {
            self.pinned.push(path.clone());
        }
        if !self.shortcuts.iter().any(|(_, existing)| existing == &path) {
            let label = path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            self.shortcuts.push((label, path));
        }
        self.save_pins(cx);
    }

    fn unpin_folder(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        self.pinned.retain(|entry| entry != path);
        // Preserve built-in home, known folders and drive entries.
        let home = dirs::home_dir().unwrap_or_default();
        let is_default = path == home
            || ["Desktop", "Documents", "Downloads", "Pictures", "Videos", "Music"]
                .iter().any(|name| path == home.join(name))
            || path.parent().is_none_or(|parent| parent == path);
        if !is_default {
            self.shortcuts.retain(|(_, entry)| entry != path);
        }
        self.save_pins(cx);
    }

    fn save_pins(&mut self, cx: &mut Context<Self>) {
        match shortcuts::config_path().and_then(|path| shortcuts::save(&path, &self.pinned)) {
            Ok(()) => self.status = "Acesso rápido atualizado".to_owned(),
            Err(error) => self.status = format!("Não foi possível salvar atalhos: {error}"),
        }
        cx.notify();
    }

    fn open_in_new_tab(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !path.is_dir() {
            self.status = "Uma nova aba precisa apontar para uma pasta".to_owned();
            cx.notify();
            return;
        }
        self.tabs.push(path.clone());
        self.active_tab = self.tabs.len() - 1;
        self.history = vec![path.clone()];
        self.history_index = 0;
        self.change_directory(path, cx);
    }

    fn show_properties(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.entries.iter().any(|entry| entry.path == path) {
            self.set_selection(path);
            self.details_open = true;
            cx.notify();
        }
    }

    fn select_all(&mut self, cx: &mut Context<Self>) {
        self.selected_paths = self
            .entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect();
        self.selected = self.selected_paths.last().cloned();
        cx.notify();
    }

    fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.selected = None;
        self.selected_paths.clear();
        cx.notify();
    }

    fn invert_selection(&mut self, cx: &mut Context<Self>) {
        let mut selected = Vec::new();
        for entry in &self.entries {
            if !self.selected_paths.contains(&entry.path) {
                selected.push(entry.path.clone());
            }
        }
        self.selected = selected.last().cloned();
        self.selected_paths = selected;
        cx.notify();
    }

    fn toggle_row_numbers(&mut self, cx: &mut Context<Self>) {
        self.show_row_numbers = !self.show_row_numbers;
        cx.notify();
    }

    fn toggle_view(&mut self, cx: &mut Context<Self>) {
        self.view_mode = match self.view_mode {
            ViewMode::Details => ViewMode::Tiles,
            ViewMode::Tiles => ViewMode::Details,
        };
        cx.notify();
    }

    fn select_entry(&mut self, path: PathBuf, additive: bool, cx: &mut Context<Self>) {
        if additive {
            if let Some(index) = self.selected_paths.iter().position(|item| item == &path) {
                self.selected_paths.remove(index);
            } else {
                self.selected_paths.push(path);
            }
        } else {
            self.selected_paths.clear();
            self.selected_paths.push(path);
        }
        self.selected = self.selected_paths.last().cloned();
        self.confirm_trash = false;
        cx.notify();
    }

    fn ensure_context_selection(&mut self, path: &std::path::Path) {
        if !self.selected_paths.iter().any(|selected| selected == path) {
            self.set_selection(path.to_path_buf());
        }
    }

    fn set_selection(&mut self, path: PathBuf) {
        self.selected_paths = vec![path.clone()];
        self.selected = Some(path);
    }

    fn open_entry(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if path.is_dir() {
            self.navigate(path, cx);
        } else if let Err(error) = open::that(&path) {
            self.status = format!("Não foi possível abrir: {error}");
            cx.notify();
        }
    }

    fn create_folder(&mut self, cx: &mut Context<Self>) {
        match filesystem::create_folder(&self.directory) {
            Ok(path) => {
                self.reload(cx);
                self.set_selection(path);
                self.status = "Pasta criada".to_owned();
                cx.notify();
            }
            Err(error) => self.report_error("Criar pasta", error, cx),
        }
    }

    fn copy_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = &self.selected {
            self.copied = Some(path.clone());
            self.clipboard_cut = false;
            self.status = "Copiado. Abra a pasta de destino e clique em Colar.".to_owned();
            cx.notify();
        }
    }

    fn cut_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = &self.selected {
            self.copied = Some(path.clone());
            self.clipboard_cut = true;
            self.status = "Recortado. Cole em outra pasta para mover.".to_owned();
            cx.notify();
        }
    }

    fn begin_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.selected.clone() else {
            return;
        };
        let name = if self.selected_paths.len() > 1 {
            "Arquivo".to_owned()
        } else {
            let Some(name) = path.file_name() else {
                return;
            };
            name.to_string_lossy().into_owned()
        };
        self.rename_input.update(cx, |state, cx| {
            state.set_value(name, window, cx);
        });
        self.rename_target = Some(path);
        cx.notify();
    }

    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.rename_target = None;
        cx.notify();
    }

    fn confirm_rename(&mut self, cx: &mut Context<Self>) {
        let Some(source) = self.rename_target.take() else {
            return;
        };
        let name = self.rename_input.read(cx).value().to_string();
        let result = if self.selected_paths.len() > 1 {
            operations::batch_rename_numbered(&self.selected_paths, &name)
        } else {
            operations::rename_item(&source, &name).map(|path| vec![path])
        };
        match result {
            Ok(paths) => {
                let count = paths.len();
                self.reload(cx);
                self.selected = paths.last().cloned();
                self.selected_paths = paths;
                self.status = format!("{count} item(ns) renomeado(s)");
                cx.notify();
            }
            Err(error) => {
                self.reload(cx);
                self.report_error("Renomear", error, cx);
            }
        }
    }

    fn export_listing(&mut self, cx: &mut Context<Self>) {
        if self.transfer_in_progress {
            return;
        }
        let directory = self.directory.clone();
        let snapshot = self.entries.clone();
        self.transfer_in_progress = true;
        self.status = "Exportando lista CSV...".to_owned();
        cx.notify();
        self.transfer_task = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move { operations::export_csv(&directory, &snapshot) })
                .await;
            _ = view.update(cx, |this, cx| {
                this.transfer_in_progress = false;
                match result {
                    Ok(path) => {
                        this.status = format!("CSV exportado: {}", path.display());
                        this.reload(cx);
                        this.status = format!("CSV exportado: {}", path.display());
                        cx.notify();
                    }
                    Err(error) => this.report_error("Exportar CSV", error, cx),
                }
            });
        }));
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        if self.transfer_in_progress {
            return;
        }
        let Some(source) = self.copied.clone() else {
            self.status = "Selecione um item e use Copiar primeiro.".to_owned();
            cx.notify();
            return;
        };
        let destination = self.directory.clone();
        let cut = self.clipboard_cut;
        self.transfer_in_progress = true;
        self.status = "Copiando arquivos em segundo plano...".to_owned();
        cx.notify();

        self.transfer_task = Some(cx.spawn(async move |view, cx| {
            let destination_for_copy = destination.clone();
            let result = cx
                .background_spawn(async move {
                    if cut {
                        operations::move_into(&source, &destination_for_copy)
                    } else {
                        filesystem::copy_into(&source, &destination_for_copy)
                    }
                })
                .await;
            _ = view.update(cx, |this, cx| {
                this.transfer_in_progress = false;
                match result {
                    Ok(path) => {
                        if cut {
                            this.copied = None;
                            this.clipboard_cut = false;
                        }
                        if this.directory == destination {
                            this.reload(cx);
                            this.set_selection(path);
                        }
                        this.status = if cut {
                            "Movimentação concluída"
                        } else {
                            "Cópia concluída"
                        }
                        .to_owned();
                        cx.notify();
                    }
                    Err(error) => this.report_error("Copiar", error, cx),
                }
            });
        }));
    }

    fn request_trash(&mut self, cx: &mut Context<Self>) {
        if self.selected.is_some() {
            self.confirm_trash = true;
            cx.notify();
        }
    }

    fn confirm_trash(&mut self, cx: &mut Context<Self>) {
        self.confirm_trash = false;
        if let Some(path) = self.selected.clone() {
            match filesystem::move_to_trash(&path) {
                Ok(()) => {
                    self.reload(cx);
                    self.status = "Item enviado para a Lixeira".to_owned();
                    cx.notify();
                }
                Err(error) => self.report_error("Lixeira", error, cx),
            }
        }
    }

    fn report_error(&mut self, label: &str, error: impl std::fmt::Display, cx: &mut Context<Self>) {
        self.status = format!("{label}: {error}");
        cx.notify();
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx: &mut App| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Light, None, cx);

            cx.bind_keys([
                KeyBinding::new("alt-left", Back, None),
                KeyBinding::new("alt-right", Forward, None),
                KeyBinding::new("alt-up", Up, None),
                KeyBinding::new("f5", Refresh, None),
                KeyBinding::new("ctrl-shift-n", NewFolder, None),
                KeyBinding::new("ctrl-c", Copy, None),
                KeyBinding::new("ctrl-v", Paste, None),
                KeyBinding::new("delete", Delete, None),
                KeyBinding::new("enter", OpenSelected, None),
                KeyBinding::new("ctrl-h", ToggleHidden, None),
                KeyBinding::new("ctrl-x", Cut, None),
                KeyBinding::new("f2", Rename, None),
                KeyBinding::new("ctrl-shift-e", ExportCsv, None),
                KeyBinding::new("ctrl-shift-l", ToggleRowNumbers, None),
                KeyBinding::new("ctrl-a", SelectAll, None),
                KeyBinding::new("escape", ClearSelection, None),
                KeyBinding::new("alt-enter", ShowProperties, None),
                KeyBinding::new("shift-enter", OpenInNewTab, None),
                KeyBinding::new("ctrl-t", NewTab, None),
                KeyBinding::new("ctrl-w", CloseTab, None),
                KeyBinding::new("alt-shift-p", ToggleDetails, None),
                KeyBinding::new("ctrl-shift-1", ToggleView, None),
            ]);

            let bounds = Bounds::centered(None, size(px(1200.), px(760.)), cx);
            if let Err(error) = gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..TitleBar::window_options()
                },
                cx,
                |window, cx| cx.new(|cx| FilesApp::new(window, cx)),
            ) {
                eprintln!("Não foi possível abrir a janela: {error}");
                cx.quit();
                return;
            }
            cx.activate(true);
        });
}
