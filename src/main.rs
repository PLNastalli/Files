//! Windows-first native file manager powered by GPUI.
//! This is an early MVP, not yet a drop-in replacement for explorer.exe.

use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    time::Duration,
};

use files::filesystem::{self, FileEntry, SortBy};
use gpui_kit::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, KeyBinding, Subscription, Window,
    WindowBounds, WindowOptions, actions, px, size,
};
use gpui_kit::component::{
    Theme, ThemeMode, TitleBar,
    input::{InputEvent, InputState},
};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

mod ui;

actions!(
    files,
    [Back, Forward, Up, Refresh, NewFolder, Copy, Paste, Delete, OpenSelected, ToggleHidden]
);

struct FilesApp {
    directory: PathBuf,
    entries: Vec<FileEntry>,
    history: Vec<PathBuf>,
    history_index: usize,
    selected: Option<PathBuf>,
    copied: Option<PathBuf>,
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
        let search_events = cx.subscribe_in(&search, window, |_, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });

        let mut result = Self {
            directory: home.clone(),
            entries: Vec::new(),
            history: vec![home],
            history_index: 0,
            selected: None,
            copied: None,
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
        };
        result.reload(cx);
        result.watch_directory();
        result.start_watcher_poll(cx);
        result
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        match filesystem::read_directory(&self.directory, self.show_hidden, self.sort) {
            Ok(entries) => {
                if self
                    .selected
                    .as_ref()
                    .is_some_and(|path| !entries.iter().any(|entry| &entry.path == path))
                {
                    self.selected = None;
                }
                self.status = format!("{} itens", entries.len());
                self.entries = entries;
            }
            Err(error) => {
                self.entries.clear();
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
        }) {
            if watcher
                .watch(&self.directory, RecursiveMode::NonRecursive)
                .is_ok()
            {
                self.watcher = Some(watcher);
                self.change_rx = Some(receiver);
            }
        }
    }

    fn start_watcher_poll(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(750)).await;
                if view
                    .update(cx, |this, cx| {
                        let (changed, error) = this
                            .change_rx
                            .as_ref()
                            .map(|rx| rx.try_iter().fold((false, None), |(changed, error), event| {
                                match event {
                                    Ok(_) => (true, error),
                                    Err(reason) => (changed, Some(reason)),
                                }
                            }))
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
        self.directory = path;
        self.selected = None;
        self.confirm_trash = false;
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
                self.selected = Some(path);
                self.status = "Pasta criada".to_owned();
                cx.notify();
            }
            Err(error) => self.report_error("Criar pasta", error, cx),
        }
    }

    fn copy_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = &self.selected {
            self.copied = Some(path.clone());
            self.status = "Copiado. Abra outra pasta e clique em Colar.".to_string();
            cx.notify();
        }
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
        self.transfer_in_progress = true;
        self.status = "Copiando arquivos em segundo plano...".to_owned();
        cx.notify();

        self.transfer_task = Some(cx.spawn(async move |view, cx| {
            let destination_for_copy = destination.clone();
            let result = cx
                .background_spawn(async move {
                    filesystem::copy_into(&source, &destination_for_copy)
                })
                .await;
            _ = view.update(cx, |this, cx| {
                this.transfer_in_progress = false;
                match result {
                    Ok(path) => {
                        if this.directory == destination {
                            this.reload(cx);
                            this.selected = Some(path);
                        }
                        this.status = "Cópia concluída".to_owned();
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
            Theme::change(ThemeMode::Dark, None, cx);

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
