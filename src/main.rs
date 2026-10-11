//! Windows-first native file manager powered by GPUI.
//! This is an early MVP, not yet a drop-in replacement for explorer.exe.

use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    time::Duration,
};

use files::filesystem::{self, EntryKind, FileEntry, SortBy};
use gpui_kit::{
    App, Bounds, ClickEvent, Context, Div, Entity, FocusHandle, KeyBinding, Render, Stateful,
    Subscription, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, size,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _, Theme, ThemeMode, Icon,
    button::{Button, ButtonVariants as _},
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle},
    input::{Input, InputEvent, InputState},
    sidebar::{Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    status_bar::StatusBar,
    toolbar::Toolbar,
    TitleBar,
};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

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
                        let changed = this
                            .change_rx
                            .as_ref()
                            .is_some_and(|rx| rx.try_iter().any(|event| event.is_ok()));
                        if changed {
                            this.reload(cx);
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
        let Some(source) = self.copied.clone() else {
            self.status = "Selecione um item e use Copiar primeiro.".to_owned();
            cx.notify();
            return;
        };
        match filesystem::copy_into(&source, &self.directory) {
            Ok(path) => {
                self.reload(cx);
                self.selected = Some(path);
                self.status = "Cópia concluída".to_owned();
                cx.notify();
            }
            Err(error) => self.report_error("Copiar", error, cx),
        }
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

    fn toolbar(&mut self, cx: &mut Context<Self>) -> Div {
        let mut row = div()
            .flex()
            .items_center()
            .gap_2()
            .h(px(56.))
            .px_4()
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(button("back", "‹").on_click(cx.listener(|this, _, _, cx| this.back(cx))))
            .child(button("forward", "›").on_click(cx.listener(|this, _, _, cx| this.forward(cx))))
            .child(button("up", "↑").on_click(cx.listener(|this, _, _, cx| this.up(cx))))
            .child(button("refresh", "↻").on_click(cx.listener(|this, _, _, cx| this.reload(cx))))
            .child(div().w(px(1.)).h_6().mx_1().bg(rgb(BORDER)))
            .child(button("new-folder", "＋ Nova pasta").on_click(cx.listener(|this, _, _, cx| this.create_folder(cx))))
            .child(button("copy", "Copiar").on_click(cx.listener(|this, _, _, cx| this.copy_selected(cx))))
            .child(button("paste", "Colar").on_click(cx.listener(|this, _, _, cx| this.paste(cx))))
            .child(button("sort", "Ordenar").on_click(cx.listener(|this, _, _, cx| {
                this.sort = this.sort.next();
                this.reload(cx);
            })))
            .child(button("hidden", "Ocultos").on_click(cx.listener(|this, _, _, cx| {
                this.show_hidden = !this.show_hidden;
                this.reload(cx);
            })));
        if self.confirm_trash {
            row = row
                .child(button("trash-yes", "Confirmar Lixeira").on_click(cx.listener(|this, _, _, cx| this.confirm_trash(cx))))
                .child(button("trash-no", "Cancelar").on_click(cx.listener(|this, _, _, cx| {
                    this.confirm_trash = false;
                    cx.notify();
                })));
        } else {
            row = row.child(button("trash", "Lixeira").on_click(cx.listener(|this, _, _, cx| this.request_trash(cx))));
        }
        row
    }

    fn sidebar(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let items = self
            .shortcuts
            .clone()
            .into_iter()
            .enumerate()
            .map(|(index, (label, path))| {
                let current = self.directory == path;
                div()
                    .id(("shortcut", index))
                    .h(px(36.))
                    .px_3()
                    .mb_1()
                    .rounded_md()
                    .flex()
                    .items_center()
                    .text_sm()
                    .text_color(if current { rgb(TEXT) } else { rgb(MUTED) })
                    .bg(if current { rgb(SELECTED) } else { rgb(SIDEBAR) })
                    .cursor_pointer()
                    .hover(|element| element.bg(rgb(HOVER)))
                    .child(format!("▣   {label}"))
                    .on_click(cx.listener(move |this, _, _, cx| this.navigate(path.clone(), cx)))
            });

        div()
            .id("sidebar-scroll")
            .w(px(220.))
            .flex_shrink_0()
            .h_full()
            .border_r_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SIDEBAR))
            .p_3()
            .overflow_y_scroll()
            .child(div().text_xs().text_color(rgb(MUTED)).px_3().pt_3().pb_3().child("ACESSO RÁPIDO"))
            .children(items)
    }

    fn breadcrumbs(&mut self, cx: &mut Context<Self>) -> Div {
        let mut segments = self
            .directory
            .ancestors()
            .filter(|path| !path.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .collect::<Vec<_>>();
        segments.reverse();

        let crumbs = segments.into_iter().enumerate().map(|(index, path)| {
            let text = path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().text_color(rgb(MUTED)).child(if index == 0 { "⌂" } else { "›" }))
                .child(
                    div()
                        .id(("crumb", index))
                        .cursor_pointer()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .hover(|element| element.bg(rgb(HOVER)))
                        .child(text)
                        .on_click(cx.listener(move |this, _, _, cx| this.navigate(path.clone(), cx))),
                )
        });

        div()
            .h(px(52.))
            .px_5()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(rgb(BORDER))
            .text_sm()
            .text_color(rgb(TEXT))
            .children(crumbs)
    }

    fn file_list(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let rows = self.entries.clone().into_iter().enumerate().map(|(index, entry)| {
            let is_selected = self.selected.as_ref() == Some(&entry.path);
            let icon = match entry.kind {
                EntryKind::Folder => "▣",
                EntryKind::File => "▤",
                EntryKind::Symlink => "↗",
            };
            let size_label = if entry.kind == EntryKind::Folder {
                "—".to_owned()
            } else {
                filesystem::display_size(entry.size)
            };
            let path = entry.path.clone();
            div()
                .id(("entry", index))
                .h(px(40.))
                .mx_3()
                .px_3()
                .rounded_sm()
                .flex()
                .items_center()
                .text_sm()
                .cursor_pointer()
                .bg(if is_selected { rgb(SELECTED) } else { rgb(BG) })
                .hover(|element| element.bg(rgb(HOVER)))
                .child(div().w(px(30.)).text_color(rgb(if entry.kind == EntryKind::Folder { ACCENT } else { MUTED })).child(icon))
                .child(div().flex_1().min_w_0().text_color(rgb(TEXT)).child(entry.name.clone()))
                .child(div().w(px(136.)).text_color(rgb(MUTED)).child(entry.kind_label()))
                .child(div().w(px(92.)).text_color(rgb(MUTED)).child(size_label))
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    if matches!(event, ClickEvent::Mouse(click) if click.up.click_count >= 2) {
                        this.open_entry(path.clone(), cx);
                    } else {
                        this.selected = Some(path.clone());
                        this.confirm_trash = false;
                        cx.notify();
                    }
                }))
        });

        div()
            .id("file-list-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .bg(rgb(BG))
            .child(
                div()
                    .h(px(38.))
                    .px_6()
                    .flex()
                    .items_center()
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(div().flex_1().child("Nome"))
                    .child(div().w(px(136.)).child("Tipo"))
                    .child(div().w(px(92.)).child("Tamanho")),
            )
            .children(rows)
    }
}

impl Render for FilesApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("files-root")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Back, _, cx| this.back(cx)))
            .on_action(cx.listener(|this, _: &Forward, _, cx| this.forward(cx)))
            .on_action(cx.listener(|this, _: &Up, _, cx| this.up(cx)))
            .on_action(cx.listener(|this, _: &Refresh, _, cx| this.reload(cx)))
            .on_action(cx.listener(|this, _: &NewFolder, _, cx| this.create_folder(cx)))
            .on_action(cx.listener(|this, _: &Copy, _, cx| this.copy_selected(cx)))
            .on_action(cx.listener(|this, _: &Paste, _, cx| this.paste(cx)))
            .on_action(cx.listener(|this, _: &Delete, _, cx| this.request_trash(cx)))
            .on_action(cx.listener(|this, _: &OpenSelected, _, cx| {
                if let Some(path) = this.selected.clone() {
                    this.open_entry(path, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleHidden, _, cx| {
                this.show_hidden = !this.show_hidden;
                this.reload(cx);
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .child(
                div()
                    .h(px(43.))
                    .px_5()
                    .flex()
                    .items_center()
                    .bg(rgb(PANEL))
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(div().text_color(rgb(ACCENT)).font_weight(gpui::FontWeight::BOLD).child("▣  Files"))
                    .child(div().ml_4().text_xs().text_color(rgb(MUTED)).child("Rust · GPUI")),
            )
            .child(self.toolbar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.sidebar(cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(self.breadcrumbs(cx))
                            .child(
                                div()
                                    .h(px(40.))
                                    .px_6()
                                    .flex()
                                    .items_center()
                                    .bg(rgb(PANEL))
                                    .text_sm()
                                    .text_color(rgb(MUTED))
                                    .child(format!("Ordenação: {}  ·  Ocultos: {}", self.sort.label(), if self.show_hidden { "sim" } else { "não" })),
                            )
                            .child(self.file_list(cx)),
                    ),
            )
            .child(
                div()
                    .h(px(32.))
                    .px_4()
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(PANEL))
                    .text_xs()
                    .text_color(rgb(MUTED))
                    .child(self.status.clone())
                    .child(div().flex_1())
                    .child(if self.selected.is_some() { "1 selecionado" } else { "Nenhum selecionado" }),
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
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

        let bounds = Bounds::centered(None, size(px(1120.), px(740.)), cx);
        if let Err(error) = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| FilesApp::new(window, cx)),
        ) {
            eprintln!("Não foi possível abrir a janela: {error}");
            cx.quit();
        } else {
            cx.activate(true);
        }
    });
}
