//! Explorer-inspired Windows 11 light presentation, implemented with GPUI Kit.
//! A cohesive palette lives here instead of scattering raw colors across widgets.

use super::*;
use chrono::{DateTime, Local};
use files::filesystem::EntryKind;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _, TitleBar,
    button::{Button, ButtonVariants as _},
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle},
    input::Input,
    status_bar::StatusBar,
    tab::{Tab, TabBar},
};
use gpui_kit::{ClickEvent, Div, Render, div, prelude::*, rgb};
use std::path::Path;

// Win11System is the reference application's default. These are approximations
// of its light-mode system colors: DevExpress resolves the original palette at runtime.
const WINDOW_BG: u32 = 0xF3F3F3;
const CANVAS: u32 = 0xFFFFFF;
const TOOLBAR_BG: u32 = 0xFBFBFB;
const SIDEBAR_BG: u32 = 0xF7F7F7;
const BORDER: u32 = 0xE5E5E5;
const TEXT: u32 = 0x202020;
const SECONDARY: u32 = 0x626262;
const HOVER: u32 = 0xF2F5F9;
const SELECTED: u32 = 0xDFEBFA;

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn display_datetime(timestamp: Option<std::time::SystemTime>) -> String {
    timestamp
        .map(|time| {
            let datetime: DateTime<Local> = time.into();
            datetime.format("%d/%m/%Y %H:%M").to_string()
        })
        .unwrap_or_else(|| "—".to_owned())
}

fn modified_date(entry: &FileEntry) -> String {
    display_datetime(entry.modified)
}

fn created_date(entry: &FileEntry) -> String {
    display_datetime(entry.created)
}

fn file_type_name(entry: &FileEntry) -> String {
    match entry.kind {
        EntryKind::Folder => "Pasta de arquivos".to_owned(),
        EntryKind::Symlink => "Atalho / link".to_owned(),
        EntryKind::File => match entry
            .path
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some(extension) if !extension.is_empty() => {
                format!("Arquivo {}", extension.to_ascii_uppercase())
            }
            _ => "Arquivo".to_owned(),
        },
    }
}

/// Thin Ribbon group: command area above, small caption along the lower edge,
/// separated from neighbouring groups like the DevExpress Office ribbon.
fn ribbon_group(label: &'static str, content: impl IntoElement) -> Div {
    div()
        .h_full()
        .px_3()
        .pt_2()
        .border_r_1()
        .border_color(rgb(BORDER))
        .flex()
        .flex_col()
        .justify_between()
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(content),
        )
        .child(
            div()
                .h(px(21.))
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(rgb(SECONDARY))
                .child(label),
        )
}

impl FilesApp {
    fn tab_strip(&mut self, cx: &mut Context<Self>) -> Div {
        let tabs = self
            .tabs
            .iter()
            .map(|path| {
                Tab::new()
                    .label(display_name(path))
                    .icon(Icon::new(IconName::Folder))
            })
            .collect::<Vec<_>>();

        div()
            .h(px(42.))
            .px_3()
            .flex()
            .items_end()
            .bg(rgb(WINDOW_BG))
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                TabBar::new("explorer-tabs")
                    .outline()
                    .selected_index(self.active_tab)
                    .max_width(px(215.))
                    .children(tabs)
                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                        this.select_tab(*index, cx);
                    }))
                    .suffix(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("tab-add")
                                    .ghost()
                                    .icon(IconName::Plus)
                                    .tooltip("Nova aba (Ctrl+T)")
                                    .on_click(cx.listener(|this, _, _, cx| this.new_tab(cx))),
                            )
                            .child(
                                Button::new("tab-close")
                                    .ghost()
                                    .icon(IconName::X)
                                    .tooltip("Fechar aba (Ctrl+W)")
                                    .disabled(self.tabs.len() <= 1)
                                    .on_click(cx.listener(|this, _, _, cx| this.close_tab(cx))),
                            ),
                    ),
            )
    }

    /// A ribbon mirrors the reference WPF application's Home/View/Themes pages.
    /// Each visible command is backed by a real action, not decorative UI.
    fn ribbon(&mut self, cx: &mut Context<Self>) -> Div {
        let tabs = div()
            .h(px(32.))
            .flex()
            .items_center()
            .bg(rgb(CANVAS))
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .id("ribbon-home")
                    .h_full()
                    .px_5()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .text_color(rgb(if self.ribbon_page == RibbonPage::Home {
                        0x0067C0
                    } else {
                        TEXT
                    }))
                    .border_b_2()
                    .border_color(rgb(if self.ribbon_page == RibbonPage::Home {
                        0x0078D4
                    } else {
                        CANVAS
                    }))
                    .child("Início")
                    .on_click(cx.listener(|this, _, _, cx| this.show_ribbon(RibbonPage::Home, cx))),
            )
            .child(
                div()
                    .id("ribbon-view")
                    .h_full()
                    .px_5()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .text_color(rgb(if self.ribbon_page == RibbonPage::View {
                        0x0067C0
                    } else {
                        TEXT
                    }))
                    .border_b_2()
                    .border_color(rgb(if self.ribbon_page == RibbonPage::View {
                        0x0078D4
                    } else {
                        CANVAS
                    }))
                    .child("Exibir")
                    .on_click(cx.listener(|this, _, _, cx| this.show_ribbon(RibbonPage::View, cx))),
            )
            .child(
                div()
                    .id("ribbon-themes")
                    .h_full()
                    .px_5()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .text_color(rgb(if self.ribbon_page == RibbonPage::Themes {
                        0x0067C0
                    } else {
                        TEXT
                    }))
                    .border_b_2()
                    .border_color(rgb(if self.ribbon_page == RibbonPage::Themes {
                        0x0078D4
                    } else {
                        CANVAS
                    }))
                    .child("Temas")
                    .on_click(
                        cx.listener(|this, _, _, cx| this.show_ribbon(RibbonPage::Themes, cx)),
                    ),
            );
        let mut groups = div()
            .h(px(100.))
            .px_2()
            .flex()
            .items_stretch()
            .bg(rgb(TOOLBAR_BG))
            .border_b_1()
            .border_color(rgb(BORDER));

        match self.ribbon_page {
            RibbonPage::Home => {
                groups = groups
                    .child(ribbon_group(
                        "Área de transferência",
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("home-paste")
                                    .ghost()
                                    .icon(IconName::ClipboardPaste)
                                    .label("Colar")
                                    .disabled(self.copied.is_none() || self.transfer_in_progress)
                                    .on_click(cx.listener(|this, _, _, cx| this.paste(cx))),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        Button::new("home-copy")
                                            .ghost()
                                            .icon(IconName::Copy)
                                            .label("Copiar")
                                            .disabled(self.selected.is_none())
                                            .on_click(
                                                cx.listener(|this, _, _, cx| {
                                                    this.copy_selected(cx)
                                                }),
                                            ),
                                    )
                                    .child(
                                        Button::new("home-cut")
                                            .ghost()
                                            .icon(IconName::Scissors)
                                            .label("Recortar")
                                            .disabled(self.selected.is_none())
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.cut_selected(cx)),
                                            ),
                                    ),
                            ),
                    ))
                    .child(ribbon_group(
                        "Organizar",
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                Button::new("home-rename")
                                    .ghost()
                                    .icon(IconName::FilePen)
                                    .label("Renomear")
                                    .disabled(self.selected.is_none())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.begin_rename(window, cx)
                                    })),
                            )
                            .child(
                                Button::new("home-trash")
                                    .ghost()
                                    .icon(IconName::Trash)
                                    .label("Excluir")
                                    .disabled(self.selected.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| this.request_trash(cx))),
                            ),
                    ))
                    .child(ribbon_group(
                        "Novo",
                        div().flex().items_center().child(
                            Button::new("home-new-folder")
                                .ghost()
                                .icon(IconName::FolderPlus)
                                .label("Nova pasta")
                                .on_click(cx.listener(|this, _, _, cx| this.create_folder(cx))),
                        ),
                    ))
                    .child(ribbon_group(
                        "Abrir",
                        div().flex().items_center().child(
                            Button::new("home-open")
                                .ghost()
                                .icon(IconName::FolderOpen)
                                .label("Abrir")
                                .disabled(self.selected.is_none())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(path) = this.selected.clone() {
                                        this.open_entry(path, cx);
                                    }
                                })),
                        ),
                    ))
                    .child(ribbon_group(
                        "Selecionar",
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                Button::new("select-all")
                                    .ghost()
                                    .icon(IconName::ListChecks)
                                    .label("Selecionar tudo")
                                    .on_click(cx.listener(|this, _, _, cx| this.select_all(cx))),
                            )
                            .child(
                                Button::new("select-none")
                                    .ghost()
                                    .icon(IconName::ListX)
                                    .label("Limpar seleção")
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.clear_selection(cx)),
                                    ),
                            )
                            .child(
                                Button::new("select-invert")
                                    .ghost()
                                    .icon(IconName::CheckCheck)
                                    .label("Inverter")
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.invert_selection(cx)),
                                    ),
                            ),
                    ));
            }
            RibbonPage::View => {
                groups = groups
                    .child(ribbon_group(
                        "Painéis",
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                Button::new("view-navigation")
                                    .ghost()
                                    .icon(IconName::PanelLeft)
                                    .label(if self.show_navigation {
                                        "Ocultar árvore"
                                    } else {
                                        "Painel de navegação"
                                    })
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.toggle_navigation(cx)),
                                    ),
                            )
                            .child(
                                Button::new("view-details")
                                    .ghost()
                                    .icon(IconName::PanelRight)
                                    .label(if self.details_open {
                                        "Ocultar detalhes"
                                    } else {
                                        "Painel de detalhes"
                                    })
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.toggle_details(cx)),
                                    ),
                            ),
                    ))
                    .child(ribbon_group(
                        "Layout",
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                Button::new("view-table")
                                    .ghost()
                                    .icon(IconName::List)
                                    .label("Detalhes")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_view_mode(ViewMode::Details, cx)
                                    })),
                            )
                            .child(
                                Button::new("view-tiles")
                                    .ghost()
                                    .icon(IconName::LayoutGrid)
                                    .label("Blocos")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_view_mode(ViewMode::Tiles, cx)
                                    })),
                            ),
                    ))
                    .child(ribbon_group(
                        "Exibir / ocultar",
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                Button::new("view-hidden")
                                    .ghost()
                                    .icon(IconName::Eye)
                                    .label(if self.show_hidden {
                                        "Ocultos: sim"
                                    } else {
                                        "Itens ocultos"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.show_hidden = !this.show_hidden;
                                        this.reload(cx);
                                    })),
                            )
                            .child(
                                Button::new("view-row-numbers")
                                    .ghost()
                                    .icon(IconName::ListOrdered)
                                    .label(if self.show_row_numbers {
                                        "Números: sim"
                                    } else {
                                        "Números de linha"
                                    })
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.toggle_row_numbers(cx)),
                                    ),
                            ),
                    ))
                    .child(ribbon_group(
                        "Organizar",
                        div().flex().items_center().child(
                            Button::new("view-sort")
                                .ghost()
                                .icon(IconName::ArrowDownAZ)
                                .label(format!("Classificar: {}", self.sort.label()))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.sort = this.sort.next();
                                    this.reload(cx);
                                })),
                        ),
                    ))
                    .child(ribbon_group(
                        "Exportar",
                        div().flex().items_center().child(
                            Button::new("view-export")
                                .ghost()
                                .icon(IconName::FileDown)
                                .label("Exportar CSV")
                                .disabled(self.transfer_in_progress)
                                .on_click(cx.listener(|this, _, _, cx| this.export_listing(cx))),
                        ),
                    ));
            }
            RibbonPage::Themes => {
                groups = groups
                    .child(ribbon_group(
                        "Tema do Windows",
                        div().flex().items_center().gap_2().child(
                            Button::new("theme-win11")
                                .icon(IconName::Sun)
                                .label("Windows 11 claro")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    Theme::change(ThemeMode::Light, None, cx);
                                    this.status = "Tema Windows 11 claro selecionado".to_owned();
                                    cx.notify();
                                })),
                        ),
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .px_4()
                            .text_xs()
                            .text_color(rgb(SECONDARY))
                            .child("Outros temas da referência ainda não foram implementados."),
                    );
            }
        }
        if self.confirm_trash {
            groups = groups.child(ribbon_group(
                "Confirmar exclusão",
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("trash-confirm")
                            .icon(IconName::Check)
                            .label("Enviar à Lixeira")
                            .on_click(cx.listener(|this, _, _, cx| this.confirm_trash(cx))),
                    )
                    .child(
                        Button::new("trash-cancel")
                            .ghost()
                            .icon(IconName::X)
                            .label("Cancelar")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.confirm_trash = false;
                                cx.notify();
                            })),
                    ),
            ));
        }

        div().flex().flex_col().child(tabs).child(groups)
    }

    fn rename_bar(&mut self, cx: &mut Context<Self>) -> Div {
        let mut bar = div();
        if self.rename_target.is_some() {
            bar = bar
                .h(px(56.))
                .px_5()
                .flex()
                .items_center()
                .gap_3()
                .border_b_1()
                .border_color(rgb(BORDER))
                .bg(rgb(TOOLBAR_BG))
                .child(div().text_sm().child(if self.selected_paths.len() > 1 {
                    "Prefixo do lote (001, 002...):"
                } else {
                    "Novo nome:"
                }))
                .child(div().w(px(360.)).child(Input::new(&self.rename_input)))
                .child(
                    Button::new("rename-save")
                        .icon(IconName::Check)
                        .label(if self.selected_paths.len() > 1 {
                            "Renomear lote"
                        } else {
                            "Salvar"
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.confirm_rename(cx))),
                )
                .child(
                    Button::new("rename-cancel")
                        .ghost()
                        .icon(IconName::X)
                        .label("Cancelar")
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_rename(cx))),
                );
        }
        bar
    }

    // The reference app has a file-tree pane, not a dashboard-style sidebar.
    // Ancestors and immediate child folders form the visible navigation hierarchy.
    fn navigation_tree(&mut self, cx: &mut Context<Self>) -> Div {
        let quick_access = self
            .shortcuts
            .clone()
            .into_iter()
            .enumerate()
            .map(|(index, (label, path))| {
                let active = self.directory == path;
                let icon = if label.starts_with("Disco") {
                    IconName::HardDrive
                } else {
                    IconName::Folder
                };
                div()
                    .id(("quick-access", index))
                    .h(px(28.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .text_sm()
                    .text_color(rgb(TEXT))
                    .bg(rgb(if active { SELECTED } else { CANVAS }))
                    .hover(|style| style.bg(rgb(HOVER)))
                    .child(Icon::new(icon).small().text_color(rgb(0xD69E42)))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| this.navigate(path.clone(), cx)))
            })
            .collect::<Vec<_>>();

        let mut ancestry = self
            .directory
            .ancestors()
            .filter(|path| !path.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .collect::<Vec<_>>();
        ancestry.reverse();
        let ancestor_count = ancestry.len();
        let mut tree_rows = ancestry
            .into_iter()
            .enumerate()
            .map(|(depth, path)| {
                let active = path == self.directory;
                div()
                    .id(("tree-ancestor", depth))
                    .h(px(29.))
                    .pl(px(11. + depth.min(5) as f32 * 14.))
                    .pr_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .text_sm()
                    .text_color(rgb(TEXT))
                    .bg(rgb(if active { SELECTED } else { CANVAS }))
                    .hover(|style| style.bg(rgb(HOVER)))
                    .child(
                        Icon::new(IconName::FolderOpen)
                            .small()
                            .text_color(rgb(0xD69E42)),
                    )
                    .child(display_name(&path))
                    .on_click(cx.listener(move |this, _, _, cx| this.navigate(path.clone(), cx)))
            })
            .collect::<Vec<_>>();

        let direct_folders = self
            .entries
            .iter()
            .filter(|entry| entry.kind == EntryKind::Folder)
            .take(80)
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>();
        tree_rows.extend(direct_folders.into_iter().enumerate().map(|(index, path)| {
            div()
                .id(("tree-child", index))
                .h(px(29.))
                .pl(px(11. + ancestor_count.min(5) as f32 * 14.))
                .pr_2()
                .flex()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .text_sm()
                .text_color(rgb(TEXT))
                .hover(|style| style.bg(rgb(HOVER)))
                .child(
                    Icon::new(IconName::Folder)
                        .small()
                        .text_color(rgb(0xD69E42)),
                )
                .child(display_name(&path))
                .on_click(cx.listener(move |this, _, _, cx| this.navigate(path.clone(), cx)))
        }));

        div()
            .id("explorer-navigation-tree")
            .w(px(224.))
            .flex_shrink_0()
            .h_full()
            .overflow_y_scroll()
            .bg(rgb(CANVAS))
            .border_r_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .h(px(30.))
                    .px_3()
                    .flex()
                    .items_center()
                    .text_xs()
                    .text_color(rgb(SECONDARY))
                    .child("Acesso rápido"),
            )
            .children(quick_access)
            .child(
                div()
                    .h(px(34.))
                    .px_3()
                    .pt_3()
                    .text_xs()
                    .text_color(rgb(SECONDARY))
                    .child("Pastas"),
            )
            .children(tree_rows)
    }

    fn navigation(&mut self, cx: &mut Context<Self>) -> Div {
        let mut segments = self
            .directory
            .ancestors()
            .filter(|part| !part.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .collect::<Vec<_>>();
        segments.reverse();
        let crumbs = segments
            .into_iter()
            .enumerate()
            .map(|(index, path)| {
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Icon::new(IconName::ChevronRight)
                            .xsmall()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        div()
                            .id(("breadcrumb", index))
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_sm()
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(HOVER)))
                            .child(display_name(&path))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.navigate(path.clone(), cx)),
                            ),
                    )
            })
            .collect::<Vec<_>>();

        div()
            .h(px(60.))
            .px_4()
            .flex()
            .items_center()
            .gap_3()
            .bg(rgb(CANVAS))
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("nav-back")
                            .ghost()
                            .icon(IconName::ArrowLeft)
                            .tooltip("Voltar")
                            .disabled(self.history_index == 0)
                            .on_click(cx.listener(|this, _, _, cx| this.back(cx))),
                    )
                    .child(
                        Button::new("nav-forward")
                            .ghost()
                            .icon(IconName::ArrowRight)
                            .tooltip("Avançar")
                            .disabled(self.history_index + 1 >= self.history.len())
                            .on_click(cx.listener(|this, _, _, cx| this.forward(cx))),
                    )
                    .child(
                        Button::new("nav-up")
                            .ghost()
                            .icon(IconName::ArrowUp)
                            .tooltip("Subir")
                            .on_click(cx.listener(|this, _, _, cx| this.up(cx))),
                    )
                    .child(
                        Button::new("nav-refresh")
                            .ghost()
                            .icon(IconName::RefreshCw)
                            .tooltip("Atualizar")
                            .on_click(cx.listener(|this, _, _, cx| this.reload(cx))),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h(px(37.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(TOOLBAR_BG))
                    .children(crumbs),
            )
            .child(
                div().w(px(275.)).flex_shrink_0().child(
                    Input::new(&self.search)
                        .prefix(Icon::new(IconName::Search).small())
                        .cleanable(true),
                ),
            )
    }

    fn file_list(&mut self, cx: &mut Context<Self>) -> Div {
        let query = self.search.read(cx).value().to_string().to_lowercase();
        let visible = self
            .entries
            .iter()
            .filter(|entry| query.is_empty() || entry.name.to_lowercase().contains(&query))
            .cloned()
            .collect::<Vec<_>>();
        let tiles = self.view_mode == ViewMode::Tiles;
        let mut list = div()
            .id("explorer-file-scroller")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .bg(rgb(CANVAS));
        if tiles {
            list = list.flex().flex_wrap().content_start().p_3();
        }

        if visible.is_empty() {
            list = list.child(
                div()
                    .h_full()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Empty::new().header(
                            EmptyHeader::new()
                                .media(
                                    EmptyMedia::new()
                                        .with_variant(EmptyMediaVariant::Icon)
                                        .child(Icon::new(IconName::FolderOpen)),
                                )
                                .title(EmptyTitle::new().child(if query.is_empty() {
                                    "Esta pasta está vazia"
                                } else {
                                    "Nenhum resultado encontrado"
                                }))
                                .description(
                                    EmptyDescription::new()
                                        .child("Os arquivos desta pasta aparecem aqui."),
                                ),
                        ),
                    ),
            );
        } else {
            for (index, entry) in visible.into_iter().enumerate() {
                let selected = self.selected_paths.contains(&entry.path);
                let folder = entry.kind == EntryKind::Folder;
                let symbol = match entry.kind {
                    EntryKind::Folder => IconName::Folder,
                    EntryKind::Symlink => IconName::Link,
                    EntryKind::File => {
                        match entry
                            .path
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .unwrap_or("")
                            .to_ascii_lowercase()
                            .as_str()
                        {
                            "png" | "jpg" | "jpeg" | "gif" | "webp" => IconName::FileImage,
                            "mp4" | "mkv" | "mov" => IconName::FileVideoCamera,
                            "mp3" | "wav" | "flac" => IconName::FileMusic,
                            "zip" | "7z" | "rar" => IconName::FileArchive,
                            "rs" | "py" | "js" | "ts" | "go" => IconName::FileCode,
                            _ => IconName::File,
                        }
                    }
                };
                let path = entry.path.clone();
                if tiles {
                    list = list.child(
                        div().id(("tile", index))
                            .w(px(155.)).h(px(120.)).m_1().p_3()
                            .flex().flex_col().items_center().justify_center().gap_2()
                            .rounded_md().cursor_pointer()
                            .bg(rgb(if selected { SELECTED } else { CANVAS }))
                            .hover(|style| style.bg(rgb(HOVER)))
                            .child(Icon::new(symbol).large()
                                .text_color(if folder { cx.theme().primary } else { cx.theme().muted_foreground }))
                            .child(div().w_full().text_center().text_sm().text_color(rgb(TEXT))
                                .overflow_hidden().child(entry.name.clone()))
                            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                                if matches!(event, ClickEvent::Mouse(click) if click.up.click_count >= 2) {
                                    this.open_entry(path.clone(), cx);
                                } else {
                                    let additive = matches!(event, ClickEvent::Mouse(click) if click.up.modifiers.control);
                                    this.select_entry(path.clone(), additive, cx);
                                }
                            })),
                    );
                } else {
                    let size = if folder {
                        "—".to_owned()
                    } else {
                        filesystem::display_size(entry.size)
                    };
                    list = list.child(
                        div().id(("file-row", index))
                            .h(px(29.)).px_3()
                            .flex().items_center().gap_2()
                            .cursor_pointer().text_sm()
                            .text_color(rgb(TEXT))
                            .bg(rgb(if selected { SELECTED } else { CANVAS }))
                            .hover(|style| style.bg(rgb(HOVER)))
                            .when(self.show_row_numbers, |this| {
                                this.child(div().w(px(32.)).text_xs().text_color(rgb(SECONDARY)).child((index + 1).to_string()))
                            })
                            .child(Icon::new(symbol)
                                .text_color(if folder { rgb(0xD69E42) } else { cx.theme().muted_foreground }))
                            .child(div().flex_1().min_w_0().child(entry.name.clone()))
                            .child(div().w(px(125.)).text_xs().text_color(rgb(SECONDARY))
                                .child(file_type_name(&entry)))
                            .child(div().w(px(83.)).text_right().text_xs().text_color(rgb(SECONDARY))
                                .child(size))
                            .child(div().w(px(145.)).text_xs().text_color(rgb(SECONDARY))
                                .child(created_date(&entry)))
                            .child(div().w(px(145.)).text_xs().text_color(rgb(SECONDARY))
                                .child(modified_date(&entry)))
                            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                                if matches!(event, ClickEvent::Mouse(click) if click.up.click_count >= 2) {
                                    this.open_entry(path.clone(), cx);
                                } else {
                                    let additive = matches!(event, ClickEvent::Mouse(click) if click.up.modifiers.control);
                                    this.select_entry(path.clone(), additive, cx);
                                }
                            })),
                    );
                }
            }
        }

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .when(!tiles, |this| {
                this.child(
                    div()
                        .h(px(30.))
                        .px_3()
                        .flex()
                        .items_center()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .text_color(rgb(SECONDARY))
                        .bg(rgb(TOOLBAR_BG))
                        .border_b_1()
                        .border_color(rgb(BORDER))
                        .when(self.show_row_numbers, |this| {
                            this.child(div().w(px(32.)).child("#"))
                        })
                        .child(div().flex_1().child("Nome"))
                        .child(div().w(px(125.)).child("Tipo"))
                        .child(div().w(px(83.)).text_right().child("Tamanho"))
                        .child(div().w(px(145.)).child("Data de criação"))
                        .child(div().w(px(145.)).child("Data de modificação")),
                )
            })
            .child(list)
    }

    fn details_panel(&mut self, cx: &mut Context<Self>) -> Div {
        let item = self.selected.as_ref().and_then(|path| {
            self.entries
                .iter()
                .find(|entry| &entry.path == path)
                .cloned()
        });
        let mut panel = div()
            .w(px(258.))
            .h_full()
            .flex_shrink_0()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .border_l_1()
            .border_color(rgb(BORDER))
            .bg(rgb(SIDEBAR_BG))
            .child(
                div()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(rgb(TEXT))
                    .child("Detalhes"),
            );

        if let Some(entry) = item {
            let icon = if entry.kind == EntryKind::Folder {
                IconName::FolderOpen
            } else {
                IconName::File
            };
            panel = panel
                .child(
                    div()
                        .pt_5()
                        .flex()
                        .justify_center()
                        .child(Icon::new(icon).text_color(cx.theme().primary).large()),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .text_color(rgb(TEXT))
                        .child(entry.name.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(SECONDARY))
                        .child(format!("Tipo: {}", entry.kind_label())),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(SECONDARY))
                        .child(format!("Tamanho: {}", filesystem::display_size(entry.size))),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(SECONDARY))
                        .child(format!("Modificado: {}", modified_date(&entry))),
                );
        } else {
            panel = panel
                .child(
                    div().pt_8().flex().justify_center().child(
                        Icon::new(IconName::FileSearch)
                            .large()
                            .text_color(cx.theme().muted_foreground),
                    ),
                )
                .child(
                    div()
                        .text_sm()
                        .text_center()
                        .text_color(rgb(SECONDARY))
                        .child("Selecione um item para ver seus detalhes."),
                );
        }
        panel
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
            .on_action(cx.listener(|this, _: &Cut, _, cx| this.cut_selected(cx)))
            .on_action(cx.listener(|this, _: &Rename, window, cx| this.begin_rename(window, cx)))
            .on_action(cx.listener(|this, _: &ExportCsv, _, cx| this.export_listing(cx)))
            .on_action(cx.listener(|this, _: &ToggleRowNumbers, _, cx| this.toggle_row_numbers(cx)))
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
            .on_action(cx.listener(|this, _: &NewTab, _, cx| this.new_tab(cx)))
            .on_action(cx.listener(|this, _: &CloseTab, _, cx| this.close_tab(cx)))
            .on_action(cx.listener(|this, _: &ToggleDetails, _, cx| this.toggle_details(cx)))
            .on_action(cx.listener(|this, _: &ToggleView, _, cx| this.toggle_view(cx)))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(CANVAS))
            .text_color(rgb(TEXT))
            .child(
                TitleBar::new().child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .pl_3()
                        .child(Icon::new(IconName::FolderOpen).text_color(cx.theme().primary))
                        .child(
                            div()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .child("Files"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(SECONDARY))
                                .child("Explorador de Arquivos"),
                        ),
                ),
            )
            .child(self.tab_strip(cx))
            .child(self.ribbon(cx))
            .child(self.navigation(cx))
            .child(self.rename_bar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .bg(rgb(CANVAS))
                    .when(self.show_navigation, |this| {
                        this.child(self.navigation_tree(cx))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(self.file_list(cx)),
                    )
                    .when(self.details_open, |this| this.child(self.details_panel(cx))),
            )
            .child(div().bg(rgb(WINDOW_BG)).child(
                StatusBar::new().left(self.status.clone()).right(
                    if self.selected_paths.is_empty() {
                        "Nenhum item selecionado".to_owned()
                    } else {
                        format!("{} item(ns) selecionado(s)", self.selected_paths.len())
                    },
                ),
            ))
    }
}
