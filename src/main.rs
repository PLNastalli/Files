//! Windows-first native file manager powered by GPUI.
//! This is an early MVP, not yet a drop-in replacement for explorer.exe.

use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    time::Duration,
};

use files::filesystem::{self, EntryKind, FileEntry, SortBy};
use gpui_kit::{
    App, Bounds, ClickEvent, Context, Div, Entity, FocusHandle, KeyBinding, Render,
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


    // GPUI Kit Toolbar: semantic keyboard navigation and consistently styled controls.
    fn toolbar(&mut self, cx: &mut Context<Self>) -> Div {
        let mut bar = Toolbar::new("files-commands")
            .child(Button::new("new-directory")
                .icon(IconName::FolderPlus)
                .label("Nova pasta")
                .on_click(cx.listener(|this, _, _, cx| this.create_folder(cx))))
            .content(div().w(px(1.)).h(px(22.)).bg(cx.theme().border))
            .child(Button::new("copy")
                .icon(IconName::Copy)
                .label("Copiar")
                .disabled(self.selected.is_none())
                .on_click(cx.listener(|this, _, _, cx| this.copy_selected(cx))))
            .child(Button::new("paste")
                .icon(IconName::ClipboardPaste)
                .label("Colar")
                .disabled(self.copied.is_none())
                .on_click(cx.listener(|this, _, _, cx| this.paste(cx))))
            .child(Button::new("trash")
                .icon(IconName::Trash)
                .label("Lixeira")
                .disabled(self.selected.is_none())
                .on_click(cx.listener(|this, _, _, cx| this.request_trash(cx))))
            .content(div().w(px(1.)).h(px(22.)).bg(cx.theme().border))
            .child(Button::new("sort")
                .icon(IconName::ArrowDownAZ)
                .label("Ordenar")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.sort = this.sort.next();
                    this.reload(cx);
                })))
            .child(Button::new("hidden")
                .icon(IconName::Eye)
                .label("Ocultos")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.show_hidden = !this.show_hidden;
                    this.reload(cx);
                })))
            .content(div().flex_1())
            .content(
                div().text_xs().text_color(cx.theme().muted_foreground)
                    .child(format!("Por {}", self.sort.label())),
            );

        if self.confirm_trash {
            bar = bar
                .child(Button::new("confirm-trash").icon(IconName::Check).label("Confirmar")
                    .on_click(cx.listener(|this, _, _, cx| this.confirm_trash(cx))))
                .child(Button::new("cancel-trash").icon(IconName::X).label("Cancelar")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.confirm_trash = false;
                        cx.notify();
                    })));
        }

        div()
            .h(px(54.))
            .px_5()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(bar)
    }

    // The navigation area uses the real GPUI Kit sidebar, not hand-painted rows.
    fn sidebar(&mut self, cx: &mut Context<Self>) -> Sidebar {
        let locations = self.shortcuts.clone().into_iter().map(|(label, path)| {
            let active = self.directory == path;
            let icon = match label.as_str() {
                "Início" => IconName::House,
                "Área de Trabalho" => IconName::Monitor,
                "Documentos" => IconName::FileText,
                "Downloads" => IconName::Download,
                "Imagens" => IconName::Image,
                "Vídeos" => IconName::Clapperboard,
                "Músicas" => IconName::Music,
                text if text.starts_with("Disco") => IconName::HardDrive,
                _ => IconName::Folder,
            };
            SidebarMenuItem::new(label)
                .icon(icon)
                .active(active)
                .on_click(cx.listener(move |this, _, _, cx| this.navigate(path.clone(), cx)))
        }).collect::<Vec<_>>();

        Sidebar::new("files-navigation").w(px(248.))
            .header(
                SidebarHeader::new()
                    .child(
                        div().flex().items_center().gap_3().px_2()
                            .child(Icon::new(IconName::FolderOpen).text_color(cx.theme().primary))
                            .child(
                                div().flex().flex_col().gap_1()
                                    .child(div().font_weight(gpui_kit::FontWeight::SEMIBOLD).child("Biblioteca"))
                                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child("Arquivos locais")),
                            ),
                    ),
            )
            .child(
                SidebarGroup::new("Acesso rápido")
                    .child(SidebarMenu::new().children(locations)),
            )
            .footer(
                SidebarFooter::new()
                    .child(
                        div().flex().items_center().gap_2()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(Icon::new(IconName::HardDrive).small())
                            .child("Armazenamento local"),
                    ),
            )
    }

    // Explorer-style navigation and real editable search input.
    fn navigation(&mut self, cx: &mut Context<Self>) -> Div {
        let mut segments = self.directory.ancestors()
            .filter(|part| !part.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .collect::<Vec<_>>();
        segments.reverse();
        let crumbs = segments.into_iter().enumerate().map(|(index, path)| {
            let label = path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(Icon::new(IconName::ChevronRight).xsmall()
                    .text_color(cx.theme().muted_foreground))
                .child(
                    div()
                        .id(("breadcrumb", index))
                        .px_2().py_1().rounded_md()
                        .text_sm()
                        .cursor_pointer()
                        .hover(|style| style.bg(cx.theme().accent))
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| this.navigate(path.clone(), cx))),
                )
        }).collect::<Vec<_>>();

        div()
            .h(px(64.))
            .px_5()
            .flex()
            .items_center()
            .gap_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div().flex().items_center().gap_1()
                    .child(Button::new("nav-back").ghost().icon(IconName::ArrowLeft)
                        .tooltip("Voltar (Alt+Esquerda)")
                        .disabled(self.history_index == 0)
                        .on_click(cx.listener(|this, _, _, cx| this.back(cx))))
                    .child(Button::new("nav-forward").ghost().icon(IconName::ArrowRight)
                        .tooltip("Avançar (Alt+Direita)")
                        .disabled(self.history_index + 1 == self.history.len())
                        .on_click(cx.listener(|this, _, _, cx| this.forward(cx))))
                    .child(Button::new("nav-up").ghost().icon(IconName::ArrowUp)
                        .tooltip("Pasta acima")
                        .on_click(cx.listener(|this, _, _, cx| this.up(cx))))
                    .child(Button::new("nav-reload").ghost().icon(IconName::RefreshCw)
                        .tooltip("Atualizar (F5)")
                        .on_click(cx.listener(|this, _, _, cx| this.reload(cx)))),
            )
            .child(
                div().flex_1().min_w_0().h(px(36.)).px_2()
                    .flex().items_center().gap_1()
                    .rounded_md().border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary)
                    .children(crumbs),
            )
            .child(
                div().w(px(250.)).flex_shrink_0()
                    .child(
                        Input::new(&self.search)
                            .prefix(Icon::new(IconName::Search).small())
                            .cleanable(true),
                    ),
            )
    }

    fn file_list(&mut self, cx: &mut Context<Self>) -> Div {
        let query = self.search.read(cx).value().to_string().to_lowercase();
        let visible: Vec<FileEntry> = self.entries.iter()
            .filter(|entry| query.is_empty() || entry.name.to_lowercase().contains(&query))
            .cloned()
            .collect();
        let mut list = div()
            .id("file-scroller")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .bg(cx.theme().background);

        if visible.is_empty() {
            let message = if query.is_empty() {
                "Esta pasta está vazia"
            } else {
                "Nenhum resultado encontrado"
            };
            list = list.child(
                div().flex_1().h_full().flex().items_center().justify_center()
                    .child(
                        Empty::new().header(
                            EmptyHeader::new()
                                .media(EmptyMedia::new()
                                    .with_variant(EmptyMediaVariant::Icon)
                                    .child(Icon::new(IconName::FolderOpen)))
                                .title(EmptyTitle::new().child(message))
                                .description(EmptyDescription::new().child(
                                    if query.is_empty() { "Adicione arquivos ou crie uma pasta." }
                                    else { "Tente pesquisar outro nome de arquivo." }
                                )),
                        ),
                    ),
            );
        } else {
            for (index, entry) in visible.into_iter().enumerate() {
                let selected = self.selected.as_ref() == Some(&entry.path);
                let folder = entry.kind == EntryKind::Folder;
                let symbol = if folder { IconName::Folder } else if entry.kind == EntryKind::Symlink {
                    IconName::Link
                } else { IconName::File };
                let size = if folder {
                    "—".to_owned()
                } else {
                    filesystem::display_size(entry.size)
                };
                let path = entry.path.clone();
                list = list.child(
                    div()
                        .id(("file-row", index))
                        .mx_3().my_0p5().px_4().h(px(43.))
                        .flex().items_center().gap_3()
                        .rounded_md().cursor_pointer()
                        .text_sm()
                        .bg(if selected { cx.theme().accent } else { cx.theme().background })
                        .hover(|style| style.bg(cx.theme().accent))
                        .child(
                            Icon::new(symbol)
                                .text_color(if folder { cx.theme().primary } else { cx.theme().muted_foreground }),
                        )
                        .child(div().flex_1().min_w_0().child(entry.name.clone()))
                        .child(div().w(px(155.)).flex_shrink_0().text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(entry.kind_label()))
                        .child(div().w(px(90.)).flex_shrink_0().text_right().text_xs()
                            .text_color(cx.theme().muted_foreground).child(size))
                        .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                            if matches!(event, ClickEvent::Mouse(click) if click.up.click_count >= 2) {
                                this.open_entry(path.clone(), cx);
                            } else {
                                this.selected = Some(path.clone());
                                this.confirm_trash = false;
                                cx.notify();
                            }
                        })),
                );
            }
        }

        div().flex_1().min_h_0().flex().flex_col()
            .child(
                div().h(px(38.)).px_7()
                    .flex().items_center()
                    .text_xs().font_weight(gpui_kit::FontWeight::MEDIUM)
                    .text_color(cx.theme().muted_foreground)
                    .border_b_1().border_color(cx.theme().border)
                    .child(div().flex_1().child("Nome"))
                    .child(div().w(px(155.)).child("Tipo"))
                    .child(div().w(px(90.)).text_right().child("Tamanho")),
            )
            .child(list)
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
            .size_full().flex().flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                TitleBar::new()
                    .child(
                        div().flex().items_center().gap_3().pl_3()
                            .child(Icon::new(IconName::FolderOpen).text_color(cx.theme().primary))
                            .child(div().font_weight(gpui_kit::FontWeight::SEMIBOLD).child("Files"))
                            .child(div().text_xs().text_color(cx.theme().muted_foreground).child("Gerenciador de arquivos")),
                    ),
            )
            .child(self.navigation(cx))
            .child(self.toolbar(cx))
            .child(
                div().flex_1().min_h_0().flex()
                    .child(self.sidebar(cx))
                    .child(
                        div().flex_1().min_w_0().flex().flex_col()
                            .child(
                                div().px_6().h(px(56.)).flex().items_center().justify_between()
                                    .child(
                                        div().flex().flex_col().gap_1()
                                            .child(
                                                div().text_base().font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                    .child(self.directory.file_name()
                                                        .map(|s| s.to_string_lossy().into_owned())
                                                        .unwrap_or_else(|| self.directory.display().to_string())),
                                            )
                                            .child(div().text_xs().text_color(cx.theme().muted_foreground)
                                                .child(format!("{} itens nesta pasta", self.entries.len()))),
                                    )
                                    .child(div().text_xs().text_color(cx.theme().muted_foreground)
                                        .child(format!("Ordenar: {}", self.sort.label()))),
                            )
                            .child(self.file_list(cx)),
                    ),
            )
            .child(
                StatusBar::new()
                    .left(self.status.clone())
                    .right(
                        if self.selected.is_some() { "1 item selecionado" } else { "Nenhum item selecionado" }
                    ),
            )
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
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..TitleBar::window_options()
                },
                cx,
                |window, cx| cx.new(|cx| FilesApp::new(window, cx)),
            )
            .expect("Não foi possível abrir a janela");
            cx.activate(true);
        });
}
