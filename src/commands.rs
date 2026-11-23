use crate::{Browser, Navigation, ICON_WIDTH};
use gtk::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::{Cell, RefCell},
    fs,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};

thread_local! {
    static FOCUS: Cell<u64> = const { Cell::new(0) };
    static PANES: RefCell<Vec<Weak<Browser>>> = const { RefCell::new(Vec::new()) };
}

pub fn register(browser: &Rc<Browser>) {
    PANES.with(|panes| {
        let mut panes = panes.borrow_mut();
        panes.retain(|pane| pane.strong_count() > 0);
        panes.push(Rc::downgrade(browser));
    });
    for widget in [
        browser.view.clone().upcast::<gtk::Widget>(),
        browser.icons.clone().upcast::<gtk::Widget>(),
        browser.address.clone().upcast(),
        browser.filter.clone().upcast(),
    ] {
        let pane = Rc::downgrade(browser);
        widget.connect_focus_in_event(move |_, _| {
            if let Some(pane) = pane.upgrade() {
                FOCUS.with(|stamp| {
                    stamp.set(stamp.get().wrapping_add(1));
                    pane.focus_stamp.set(stamp.get());
                });
            }
            glib::Propagation::Proceed
        });
    }
}

fn active(window: &gtk::ApplicationWindow, tabs: &gtk::Notebook) -> Option<Rc<Browser>> {
    let page = tabs.nth_page(tabs.current_page())?;
    let focus = window.focused_widget();
    PANES.with(|panes| {
        let panes = panes.borrow();
        let mut visible = panes.iter().filter_map(Weak::upgrade).filter(|pane| {
            pane.root
                .upgrade()
                .is_some_and(|root| root.is_ancestor(&page))
        });
        visible
            .find(|pane| {
                focus.as_ref().is_some_and(|focus| {
                    pane.root
                        .upgrade()
                        .is_some_and(|root| focus.is_ancestor(&root))
                })
            })
            .or_else(|| {
                panes
                    .iter()
                    .filter_map(Weak::upgrade)
                    .filter(|pane| {
                        pane.root
                            .upgrade()
                            .is_some_and(|root| root.is_ancestor(&page))
                    })
                    .max_by_key(|pane| pane.focus_stamp.get())
            })
    })
}

struct Item {
    group: &'static str,
    label: &'static str,
    action: &'static str,
    keys: &'static [&'static str],
}

pub fn install(
    window: &gtk::ApplicationWindow,
    app: &gtk::Application,
    header: &gtk::HeaderBar,
    navigation: &Navigation,
    sidebar: &gtk::ToggleButton,
) {
    let file = tr!("Arquivo", "File");
    let edit = tr!("Editar", "Edit");
    let view = tr!("Exibir", "View");
    let go = tr!("Ir", "Go");
    let help = tr!("Ajuda", "Help");
    let items = [
        Item {
            group: file,
            label: tr!("Nova aba", "New Tab"),
            action: "new-tab",
            keys: &["<Primary>t"],
        },
        Item {
            group: file,
            label: tr!("Nova janela", "New Window"),
            action: "new-window",
            keys: &["<Primary>n"],
        },
        Item {
            group: file,
            label: tr!("Nova pasta", "New Folder"),
            action: "new-folder",
            keys: &["<Primary><Shift>n"],
        },
        Item {
            group: file,
            label: tr!("Criar arquivo vazio", "Create Empty File"),
            action: "empty-file",
            keys: &[],
        },
        Item {
            group: file,
            label: tr!("Abrir", "Open"),
            action: "open",
            keys: &["<Primary>o"],
        },
        Item {
            group: file,
            label: tr!("Abrir com…", "Open With…"),
            action: "open-with",
            keys: &[],
        },
        Item {
            group: file,
            label: tr!("Abrir em nova aba", "Open in New Tab"),
            action: "open-tab",
            keys: &[],
        },
        Item {
            group: file,
            label: tr!("Abrir em nova janela", "Open in New Window"),
            action: "open-window",
            keys: &[],
        },
        Item {
            group: file,
            label: tr!("Propriedades", "Properties"),
            action: "properties",
            keys: &["<Alt>Return"],
        },
        Item {
            group: file,
            label: tr!("Fechar aba", "Close Tab"),
            action: "close-tab",
            keys: &["<Primary>w"],
        },
        Item {
            group: file,
            label: tr!("Fechar janela", "Close Window"),
            action: "close-window",
            keys: &["<Primary>q"],
        },
        Item {
            group: edit,
            label: tr!("Recortar", "Cut"),
            action: "cut",
            keys: &["<Primary>x"],
        },
        Item {
            group: edit,
            label: tr!("Copiar", "Copy"),
            action: "copy",
            keys: &["<Primary>c"],
        },
        Item {
            group: edit,
            label: tr!("Colar", "Paste"),
            action: "paste",
            keys: &["<Primary>v"],
        },
        Item {
            group: edit,
            label: tr!("Selecionar tudo", "Select All"),
            action: "select-all",
            keys: &["<Primary>a"],
        },
        Item {
            group: edit,
            label: tr!("Selecionar por padrão…", "Select by Pattern…"),
            action: "select-pattern",
            keys: &["<Primary>s"],
        },
        Item {
            group: edit,
            label: tr!("Copiar caminho", "Copy Path"),
            action: "copy-path",
            keys: &["<Primary><Shift>c"],
        },
        Item {
            group: edit,
            label: tr!("Fixar nas pastas favoritas", "Add to Shortcuts"),
            action: "pin",
            keys: &["<Primary>d"],
        },
        Item {
            group: edit,
            label: tr!("Inverter seleção", "Invert Selection"),
            action: "invert",
            keys: &["<Primary>i"],
        },
        Item {
            group: edit,
            label: tr!("Renomear", "Rename"),
            action: "rename",
            keys: &["F2"],
        },
        Item {
            group: edit,
            label: tr!("Mover para a lixeira", "Move to Trash"),
            action: "trash",
            keys: &["Delete", "KP_Delete"],
        },
        Item {
            group: edit,
            label: tr!("Excluir permanentemente…", "Delete Permanently…"),
            action: "delete",
            keys: &["<Shift>Delete", "<Shift>KP_Delete"],
        },
        Item {
            group: edit,
            label: tr!("Configurações", "Settings"),
            action: "settings",
            keys: &[],
        },
        Item {
            group: view,
            label: tr!("Recarregar", "Reload"),
            action: "reload",
            keys: &["<Primary>r", "F5"],
        },
        Item {
            group: view,
            label: tr!("Mostrar arquivos ocultos", "Show Hidden Files"),
            action: "hidden",
            keys: &["<Primary>h"],
        },
        Item {
            group: view,
            label: tr!("Barra lateral", "Sidebar"),
            action: "sidebar",
            keys: &["<Primary>b"],
        },
        Item {
            group: view,
            label: tr!("Painel duplo", "Split View"),
            action: "split",
            keys: &["F3"],
        },
        Item {
            group: view,
            label: tr!("Barra de status", "Status Bar"),
            action: "statusbar",
            keys: &[],
        },
        Item {
            group: view,
            label: tr!("Aumentar zoom", "Zoom In"),
            action: "zoom-in",
            keys: &["<Primary>plus", "<Primary>equal", "<Primary>KP_Add"],
        },
        Item {
            group: view,
            label: tr!("Diminuir zoom", "Zoom Out"),
            action: "zoom-out",
            keys: &["<Primary>minus", "<Primary>KP_Subtract"],
        },
        Item {
            group: view,
            label: tr!("Zoom normal", "Normal Size"),
            action: "zoom-reset",
            keys: &["<Primary>0", "<Primary>KP_0"],
        },
        Item {
            group: view,
            label: tr!("Tela cheia", "Fullscreen"),
            action: "fullscreen",
            keys: &["F11"],
        },
        Item {
            group: view,
            label: tr!("Filtrar nesta pasta", "Filter this Folder"),
            action: "filter",
            keys: &["<Primary>f"],
        },
        Item {
            group: go,
            label: tr!("Voltar", "Back"),
            action: "back",
            keys: &["<Alt>Left"],
        },
        Item {
            group: go,
            label: tr!("Avançar", "Forward"),
            action: "forward",
            keys: &["<Alt>Right"],
        },
        Item {
            group: go,
            label: tr!("Pasta superior", "Parent Folder"),
            action: "up",
            keys: &["<Alt>Up"],
        },
        Item {
            group: go,
            label: tr!("Pasta pessoal", "Home Folder"),
            action: "home",
            keys: &["<Alt>Home"],
        },
        Item {
            group: go,
            label: tr!("Sistema de arquivos", "File System"),
            action: "root",
            keys: &[],
        },
        Item {
            group: go,
            label: tr!("Área de trabalho", "Desktop"),
            action: "desktop",
            keys: &[],
        },
        Item {
            group: go,
            label: "Downloads",
            action: "downloads",
            keys: &[],
        },
        Item {
            group: go,
            label: tr!("Abrir terminal aqui", "Open Terminal Here"),
            action: "terminal",
            keys: &[],
        },
        Item {
            group: go,
            label: tr!("Abrir localização", "Open Location"),
            action: "location",
            keys: &["<Primary>l"],
        },
        Item {
            group: go,
            label: tr!("Aba anterior", "Previous Tab"),
            action: "previous-tab",
            keys: &["<Primary>Page_Up", "<Primary><Shift>Tab"],
        },
        Item {
            group: go,
            label: tr!("Próxima aba", "Next Tab"),
            action: "next-tab",
            keys: &["<Primary>Page_Down", "<Primary>Tab"],
        },
        Item {
            group: help,
            label: tr!("Sobre", "About"),
            action: "about",
            keys: &[],
        },
    ];
    for group in [file, edit, view, go, help] {
        let button = gtk::MenuButton::new();
        button.set_label(group);
        let model = gio::Menu::new();
        for item in items.iter().filter(|item| item.group == group) {
            let detailed = format!("win.{}", item.action);
            model.append(Some(item.label), Some(&detailed));
            let action = gio::SimpleAction::new(item.action, None);
            let owner = window.downgrade();
            let navigation = navigation.clone();
            let sidebar = sidebar.downgrade();
            let name = item.action;
            action.connect_activate(move |_, _| {
                if let Some(window) = owner.upgrade() {
                    dispatch(&window, &navigation, &sidebar, name);
                }
            });
            window.add_action(&action);
            app.set_accels_for_action(&detailed, item.keys);
        }
        button.set_menu_model(Some(&model));
        header.pack_start(&button);
    }
}

fn dispatch(
    window: &gtk::ApplicationWindow,
    navigation: &Navigation,
    sidebar: &glib::WeakRef<gtk::ToggleButton>,
    name: &str,
) {
    // Keep file accelerators from acting on the selection while editing a text field.
    if let Some(entry) = window
        .focused_widget()
        .and_then(|widget| widget.downcast::<gtk::Entry>().ok())
    {
        match name {
            "copy" => entry.emit_copy_clipboard(),
            "cut" => entry.emit_cut_clipboard(),
            "paste" => entry.emit_paste_clipboard(),
            "select-all" => entry.select_region(0, -1),
            "trash" => entry.emit_delete_from_cursor(gtk::DeleteType::Chars, 1),
            "delete" => return,
            _ => return dispatch_file(window, navigation, sidebar, name),
        }
        return;
    }
    dispatch_file(window, navigation, sidebar, name);
}

fn dispatch_file(
    window: &gtk::ApplicationWindow,
    navigation: &Navigation,
    sidebar: &glib::WeakRef<gtk::ToggleButton>,
    name: &str,
) {
    let (Some(stack), Some(tabs)) = (navigation.stack.upgrade(), navigation.tabs.upgrade()) else {
        return;
    };
    let pane = if stack.visible_child_name().as_deref() == Some("files") {
        active(window, &tabs)
    } else {
        None
    };
    let current = pane
        .as_ref()
        .map(|pane| pane.folder.borrow().clone())
        .unwrap_or_else(glib::home_dir);
    match name {
        "new-tab" => navigation.open(&current),
        "new-window" => {
            if let Err(error) = new_window(&current) {
                crate::show_error(window, &error.to_string());
            }
        }
        "close-tab" => {
            if let Some(index) = tabs.current_page() {
                tabs.remove_page(Some(index));
            }
            if tabs.n_pages() == 0 {
                stack.set_visible_child_name("home");
            }
        }
        "close-window" => window.close(),
        "settings" => crate::preferences::settings(window),
        "sidebar" => {
            if let Some(sidebar) = sidebar.upgrade() {
                sidebar.set_active(!sidebar.is_active());
            }
        }
        "fullscreen" => {
            if window
                .window()
                .is_some_and(|window| window.state().contains(gtk::gdk::WindowState::FULLSCREEN))
            {
                window.unfullscreen();
            } else {
                window.fullscreen();
            }
        }
        "home" | "root" | "desktop" | "downloads" => {
            let target = match name {
                "root" => Some(PathBuf::from("/")),
                "desktop" => glib::user_special_dir(glib::UserDirectory::Desktop),
                "downloads" => glib::user_special_dir(glib::UserDirectory::Downloads),
                _ => Some(glib::home_dir()),
            };
            let Some(target) = target else {
                return;
            };
            if let Some(pane) = pane {
                pane.navigate(&target, true);
            } else {
                navigation.open(&target);
            }
        }
        "location" | "filter" if pane.is_none() => {
            navigation.open(&current);
            if let Some(pane) = active(window, &tabs) {
                pane.dispatch(name);
            }
        }
        "previous-tab" => tabs.prev_page(),
        "next-tab" => tabs.next_page(),
        "split" => {
            if let Some(page) = tabs.nth_page(tabs.current_page()) {
                if let Some(controls) = page
                    .downcast::<gtk::Box>()
                    .ok()
                    .and_then(|page| page.children().first().cloned())
                    .and_then(|widget| widget.downcast::<gtk::Box>().ok())
                {
                    if let Some(toggle) = controls
                        .children()
                        .into_iter()
                        .find_map(|child| child.downcast::<gtk::ToggleButton>().ok())
                    {
                        toggle.set_active(!toggle.is_active());
                    }
                }
            }
        }
        "open-tab" | "open-window" => {
            if let Some(pane) = pane {
                let selected = pane.selected();
                if let [path] = selected.as_slice() {
                    if path.is_dir() {
                        if name == "open-tab" {
                            navigation.open(path);
                        } else if let Err(error) = new_window(path) {
                            crate::show_error(window, &error.to_string());
                        }
                    }
                }
            }
        }
        "about" => {
            let dialog = gtk::AboutDialog::new();
            dialog.set_transient_for(Some(window));
            dialog.set_program_name("Alpha File Manager");
            dialog.set_version(Some(env!("CARGO_PKG_VERSION")));
            dialog.set_website(Some("https://github.com/j-c-fstk-dev/alpha-file-manager"));
            dialog.run();
            dialog.close();
        }
        _ => {
            if let Some(pane) = pane {
                pane.dispatch(name);
            }
        }
    }
}

pub fn new_window(path: &Path) -> std::io::Result<()> {
    std::process::Command::new(std::env::current_exe()?)
        .arg(path)
        .spawn()?;
    Ok(())
}

pub fn terminal(path: &Path) -> std::io::Result<()> {
    for (program, args) in [
        ("exo-open", &["--launch", "TerminalEmulator"][..]),
        ("x-terminal-emulator", &[]),
    ] {
        match std::process::Command::new(program)
            .args(args)
            .current_dir(path)
            .spawn()
        {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        tr!("Terminal não encontrado.", "Terminal not found."),
    ))
}

impl Browser {
    pub fn dispatch(self: &Rc<Self>, name: &str) {
        if let Some(button) = self.commands.borrow().get(name).cloned() {
            button.clicked();
            return;
        }
        match name {
            "reload" => self.refresh(),
            "hidden" => self.hidden.set_active(!self.hidden.is_active()),
            "location" => {
                self.address.grab_focus();
                self.address.select_region(0, -1);
            }
            "filter" => {
                self.filter.grab_focus();
                self.filter.select_region(0, -1);
            }
            "select-all" => self.select_all_rows(),
            "select-pattern" => self.select_pattern(),
            "copy-path" => {
                let paths = self.selected();
                let paths = if paths.is_empty() {
                    vec![self.folder.borrow().clone()]
                } else {
                    paths
                };
                let text = paths
                    .iter()
                    .map(|path| path.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("\n");
                gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD).set_text(&text);
            }
            "pin" => {
                for path in self.selected().iter().filter(|path| path.is_dir()) {
                    if let Err(error) = crate::preferences::pin(path) {
                        self.status.set_text(&error.to_string());
                        break;
                    }
                }
            }
            "terminal" => {
                if let Err(error) = terminal(&self.folder.borrow()) {
                    self.status.set_text(&error.to_string());
                }
            }
            "statusbar" => self.status.set_visible(!self.status.is_visible()),
            "zoom-in" | "zoom-out" | "zoom-reset" => {
                let size = match name {
                    "zoom-in" => (self.zoom.get() + 1).min(24),
                    "zoom-out" => (self.zoom.get() - 1).max(9),
                    _ => 13,
                };
                self.zoom.set(size);
                self.icons.set_item_width(ICON_WIDTH * size / 13);
                for column in self.view.columns() {
                    for renderer in column
                        .cells()
                        .into_iter()
                        .filter_map(|cell| cell.downcast::<gtk::CellRendererText>().ok())
                    {
                        renderer.set_property("size-points", f64::from(size) * 0.75);
                    }
                }
                self.view.queue_resize();
                self.icons.queue_resize();
            }
            "delete" => self.delete_selected(),
            "invert" => {
                // Visible rows only: the filtered model is what the user sees.
                let mut rows = Vec::new();
                self.filtered().foreach(|_, path, _| {
                    rows.push(path.clone());
                    false
                });
                for row in rows {
                    if self.row_selected(&row) {
                        self.unselect_row(&row);
                    } else {
                        self.select_row(&row);
                    }
                }
            }
            "empty-file" => {
                if let Some(name) =
                    self.ask_name(tr!("Criar arquivo vazio", "Create Empty File"), "")
                {
                    let file = self.folder.borrow().join(name);
                    self.finish(
                        fs::OpenOptions::new()
                            .create_new(true)
                            .write(true)
                            .open(file),
                    );
                }
            }
            name => {
                let selected = self.selected();
                if let [path] = selected.as_slice() {
                    match name {
                        "open" => self.open(path),
                        "open-with" if !path.is_dir() => self.open_with(path),
                        "properties" => self.properties(path),
                        _ => {}
                    }
                }
            }
        }
    }

    fn select_pattern(&self) {
        let dialog = gtk::Dialog::with_buttons(
            Some(tr!("Selecionar por padrão", "Select by Pattern")),
            self.window().as_ref(),
            gtk::DialogFlags::MODAL,
            &[
                (tr!("Cancelar", "Cancel"), gtk::ResponseType::Cancel),
                (tr!("Selecionar", "Select"), gtk::ResponseType::Accept),
            ],
        );
        dialog.set_default_response(gtk::ResponseType::Accept);
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some("*.png"));
        entry.set_activates_default(true);
        dialog.content_area().set_border_width(12);
        dialog.content_area().add(&entry);
        dialog.show_all();
        let response = dialog.run();
        let pattern = entry.text();
        dialog.close();
        if response != gtk::ResponseType::Accept || pattern.is_empty() {
            return;
        }
        let filter = gtk::FileFilter::new();
        filter.add_pattern(&pattern);
        self.unselect_all_rows();
        let mut matches = Vec::new();
        self.filtered().foreach(|model, row, iter| {
            if let Ok(name) = model.value(iter, 1).get::<String>() {
                if filter.filter(&gtk::FileFilterInfo::new(None, None, Some(&name), None)) {
                    matches.push(row.clone());
                }
            }
            false
        });
        for row in matches {
            self.select_row(&row);
        }
    }

    fn delete_selected(self: &Rc<Self>) {
        let selected = self.selected();
        if selected.is_empty() {
            return;
        }
        let confirmation = gtk::MessageDialog::new(
            self.window().as_ref(),
            gtk::DialogFlags::MODAL,
            gtk::MessageType::Warning,
            gtk::ButtonsType::Cancel,
            &trf!(
                "Excluir permanentemente {} item(ns)?",
                "Permanently delete {} item(s)?",
                selected.len()
            ),
        );
        confirmation.add_button(tr!("Excluir", "Delete"), gtk::ResponseType::Accept);
        confirmation.set_default_response(gtk::ResponseType::Cancel);
        let response = confirmation.run();
        confirmation.close();
        if response == gtk::ResponseType::Accept {
            let result = selected.iter().try_for_each(|path| {
                let entry = fs::symlink_metadata(path)?;
                if entry.is_dir() {
                    fs::remove_dir_all(path)
                } else {
                    fs::remove_file(path)
                }
            });
            self.finish(result);
        }
    }

    pub fn open_with(&self, path: &Path) {
        let file = gio::File::for_path(path);
        let dialog =
            gtk::AppChooserDialog::new(self.window().as_ref(), gtk::DialogFlags::MODAL, &file);
        let response = dialog.run();
        let handler = dialog.app_info();
        dialog.close();
        if let (gtk::ResponseType::Ok, Some(handler)) = (response, handler) {
            if let Err(error) = handler.launch(&[file], gio::AppLaunchContext::NONE) {
                self.status.set_text(&error.to_string());
            }
        }
    }
}
