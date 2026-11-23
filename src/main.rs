#[macro_use]
mod i18n;
mod banner;
mod commands;
mod preferences;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Default)]
struct Transfer {
    paths: Vec<PathBuf>,
    moving: bool,
}

struct Browser {
    root: glib::WeakRef<gtk::Box>,
    view: gtk::TreeView,
    store: gtk::ListStore,
    paths: Rc<RefCell<Vec<PathBuf>>>,
    history: Rc<RefCell<Vec<PathBuf>>>,
    forward: RefCell<Vec<PathBuf>>,
    commands: RefCell<HashMap<&'static str, gtk::Button>>,
    transfer: Rc<RefCell<Transfer>>,
    folder: Rc<RefCell<PathBuf>>,
    address: gtk::Entry,
    filter: gtk::SearchEntry,
    hidden: gtk::ToggleButton,
    status: gtk::Label,
    sort: Cell<(i32, bool)>,
    zoom: Cell<i32>,
    focus_stamp: Cell<u64>,
    thumbnails: RefCell<VecDeque<i32>>,
    thumbnail_pending: Cell<bool>,
}

impl Browser {
    fn new(path: &Path, transfer: Rc<RefCell<Transfer>>) -> (Rc<Self>, gtk::Box) {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.set_border_width(10);
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        let back = icon_button("go-previous-symbolic", tr!("Voltar", "Back"));
        let forward = icon_button("go-next-symbolic", tr!("Avançar", "Forward"));
        let up = icon_button("go-up-symbolic", tr!("Pasta superior", "Parent Folder"));
        let address = gtk::Entry::new();
        toolbar.pack_start(&back, false, false, 0);
        toolbar.pack_start(&forward, false, false, 0);
        toolbar.pack_start(&up, false, false, 0);
        toolbar.pack_start(&address, true, true, 0);
        root.pack_start(&toolbar, false, false, 0);

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        let filter = gtk::SearchEntry::new();
        filter.set_placeholder_text(Some(tr!("Filtrar nesta pasta", "Filter this Folder")));
        let hidden = gtk::ToggleButton::new();
        hidden.add(&gtk::Image::from_icon_name(
            Some("view-reveal-symbolic"),
            gtk::IconSize::Button,
        ));
        label_control(
            &hidden,
            tr!("Mostrar arquivos ocultos", "Show Hidden Files"),
        );
        let create = icon_button("folder-new-symbolic", tr!("Nova pasta", "New Folder"));
        let rename = icon_button("document-edit-symbolic", tr!("Renomear", "Rename"));
        let trash = icon_button("user-trash-symbolic", tr!("Lixeira", "Trash"));
        let copy = icon_button("edit-copy-symbolic", tr!("Copiar", "Copy"));
        let cut = icon_button("edit-cut-symbolic", tr!("Recortar", "Cut"));
        let paste = icon_button("edit-paste-symbolic", tr!("Colar", "Paste"));
        actions.pack_start(&filter, true, true, 0);
        for button in [
            hidden.upcast_ref(),
            &create,
            &rename,
            &copy,
            &cut,
            &paste,
            &trash,
        ] {
            actions.pack_start(button, false, false, 0);
        }
        root.pack_start(&actions, false, false, 0);

        let store = gtk::ListStore::new(&[
            u32::static_type(),
            String::static_type(),
            String::static_type(),
            String::static_type(),
            String::static_type(),
            String::static_type(),
            String::static_type(),
            gtk::gdk_pixbuf::Pixbuf::static_type(),
        ]);
        let view = gtk::TreeView::with_model(&store);
        view.selection().set_mode(gtk::SelectionMode::Multiple);
        for (index, name) in [
            (1, tr!("Nome", "Name")),
            (2, tr!("Tipo", "Type")),
            (3, tr!("Tamanho", "Size")),
            (5, tr!("Criação", "Created")),
            (6, tr!("Modificação", "Modified")),
        ] {
            let column = gtk::TreeViewColumn::new();
            let renderer = gtk::CellRendererText::new();
            column.set_title(name);
            column.set_resizable(true);
            column.set_expand(index == 1);
            match index {
                2 => column.set_min_width(110),
                3 => column.set_min_width(76),
                5 | 6 => column.set_min_width(100),
                _ => {}
            }
            column.set_clickable(true);
            column.set_sort_indicator(index == 1);
            column.set_sort_order(gtk::SortType::Ascending);
            renderer.set_property("ypad", 5u32);
            renderer.set_ellipsize(gtk::pango::EllipsizeMode::End);
            if index == 1 {
                let icon = gtk::CellRendererPixbuf::new();
                icon.set_property("xpad", 8u32);
                gtk::prelude::TreeViewColumnExt::pack_start(&column, &icon, false);
                gtk::prelude::TreeViewColumnExt::set_cell_data_func(
                    &column,
                    &icon,
                    Some(Box::new(|_, renderer, model, iter| {
                        let preview = model
                            .value(iter, 7)
                            .get::<Option<gtk::gdk_pixbuf::Pixbuf>>()
                            .unwrap_or_default();
                        let name = model.value(iter, 4).get::<String>().unwrap_or_default();
                        renderer.set_property("pixbuf", Option::<gtk::gdk_pixbuf::Pixbuf>::None);
                        renderer.set_property("icon-name", Option::<String>::None);
                        if let Some(preview) = preview {
                            renderer.set_property("pixbuf", preview);
                        } else {
                            renderer.set_property("icon-name", name);
                        }
                    })),
                );
            }
            gtk::prelude::TreeViewColumnExt::pack_start(&column, &renderer, true);
            gtk::prelude::TreeViewColumnExt::add_attribute(&column, &renderer, "text", index);
            view.append_column(&column);
        }
        let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        scroll.style_context().add_class("file-surface");
        scroll.add(&view);
        root.pack_start(&scroll, true, true, 0);
        let status = gtk::Label::new(None);
        status.style_context().add_class("muted");
        status.set_xalign(0.0);
        status.set_ellipsize(gtk::pango::EllipsizeMode::End);
        root.pack_start(&status, false, false, 0);

        let browser = Rc::new(Self {
            root: root.downgrade(),
            view,
            store,
            paths: Default::default(),
            history: Default::default(),
            forward: Default::default(),
            commands: Default::default(),
            transfer: transfer.clone(),
            folder: Rc::new(RefCell::new(path.to_owned())),
            address,
            filter,
            hidden,
            status,
            sort: Cell::new((1, false)),
            zoom: Cell::new(13),
            focus_stamp: Cell::default(),
            thumbnails: Default::default(),
            thumbnail_pending: Cell::new(false),
        });
        let lifetime = browser.clone();
        root.connect_destroy(move |_| {
            let _ = &lifetime;
        });
        browser.address.set_text(&path.to_string_lossy());
        browser.refresh();
        for (id, column) in [1, 2, 3, 5, 6].into_iter().zip(browser.view.columns()) {
            let state = Rc::downgrade(&browser);
            column.connect_clicked(move |_| {
                let Some(state) = state.upgrade() else {
                    return;
                };
                let (old, descending) = state.sort.get();
                let next = old == id && !descending;
                state.sort.set((id, next));
                for (index, col) in [1, 2, 3, 5, 6].into_iter().zip(state.view.columns()) {
                    col.set_sort_indicator(index == id);
                    col.set_sort_order(if next {
                        gtk::SortType::Descending
                    } else {
                        gtk::SortType::Ascending
                    });
                }
                state.refresh();
            });
        }
        let state = Rc::downgrade(&browser);
        browser.view.connect_map(move |_| {
            if let Some(state) = state.upgrade() {
                state.queue_thumbnails();
            }
        });
        let state = Rc::downgrade(&browser);
        scroll.vadjustment().connect_value_changed(move |_| {
            if let Some(state) = state.upgrade() {
                state.queue_thumbnails();
            }
        });
        let state = Rc::downgrade(&browser);
        browser.view.connect_size_allocate(move |_, _| {
            if let Some(state) = state.upgrade() {
                state.queue_thumbnails();
            }
        });

        let state = Rc::downgrade(&browser);
        back.connect_clicked(move |_| {
            let Some(state) = state.upgrade() else {
                return;
            };
            let previous = state.history.borrow_mut().pop();
            if let Some(path) = previous {
                let current = state.folder.borrow().clone();
                if state.navigate(&path, false) {
                    state.forward.borrow_mut().push(current);
                } else {
                    state.history.borrow_mut().push(path);
                }
            }
        });
        let state = Rc::downgrade(&browser);
        forward.connect_clicked(move |_| {
            let Some(state) = state.upgrade() else {
                return;
            };
            let next = state.forward.borrow_mut().pop();
            if let Some(path) = next {
                let current = state.folder.borrow().clone();
                if state.navigate(&path, false) {
                    state.history.borrow_mut().push(current);
                } else {
                    state.forward.borrow_mut().push(path);
                }
            }
        });
        let state = Rc::downgrade(&browser);
        up.connect_clicked(move |_| {
            let Some(state) = state.upgrade() else {
                return;
            };
            let current = state.folder.borrow().clone();
            if let Some(parent) = current.parent() {
                state.navigate(parent, true);
            }
        });
        let state = Rc::downgrade(&browser);
        browser.address.connect_activate(move |entry| {
            let Some(state) = state.upgrade() else {
                return;
            };
            let text = entry.text();
            let path = match text.as_str() {
                "~" => glib::home_dir(),
                text if text.starts_with("~/") => glib::home_dir().join(&text[2..]),
                text => state.folder.borrow().join(text),
            };
            state.navigate(&path, true);
        });
        let state = Rc::downgrade(&browser);
        browser.filter.connect_search_changed(move |_| {
            if let Some(state) = state.upgrade() {
                state.refresh();
            }
        });
        let state = Rc::downgrade(&browser);
        browser.hidden.connect_toggled(move |_| {
            if let Some(state) = state.upgrade() {
                state.refresh();
            }
        });
        let state = Rc::downgrade(&browser);
        browser.view.connect_row_activated(move |_, row, _| {
            let Some(state) = state.upgrade() else {
                return;
            };
            if let Some(path) = state.path_at(row) {
                state.open(&path);
            }
        });
        let state = Rc::downgrade(&browser);
        create.connect_clicked(move |_| {
            let Some(state) = state.upgrade() else {
                return;
            };
            if let Some(name) = state.ask_name(tr!("Nova pasta", "New Folder"), "") {
                state.finish(fs::create_dir(state.folder.borrow().join(name)));
            }
        });
        let state = Rc::downgrade(&browser);
        rename.connect_clicked(move |_| {
            let Some(state) = state.upgrade() else {
                return;
            };
            let files = state.selected();
            let [source] = files.as_slice() else {
                state.status.set_text(tr!(
                    "Selecione um item para renomear.",
                    "Select one item to rename."
                ));
                return;
            };
            let Some(old_name) = source.file_name() else {
                return;
            };
            if let Some(name) =
                state.ask_name(tr!("Renomear", "Rename"), &old_name.to_string_lossy())
            {
                let target = source.with_file_name(name);
                if target == *source {
                    return;
                }
                // GIO's display-name operation refuses to replace an existing file.
                let result = gio::File::for_path(source).set_display_name(
                    &target.file_name().unwrap().to_string_lossy(),
                    gio::Cancellable::NONE,
                );
                state.finish(result);
            }
        });
        let state = Rc::downgrade(&browser);
        trash.connect_clicked(move |_| {
            let Some(state) = state.upgrade() else {
                return;
            };
            let files = state.selected();
            if files.is_empty() {
                return;
            }
            let dialog = gtk::MessageDialog::new(
                state.window().as_ref(),
                gtk::DialogFlags::MODAL,
                gtk::MessageType::Question,
                gtk::ButtonsType::OkCancel,
                &trf!(
                    "Mover {} item(ns) para a lixeira?",
                    "Move {} item(s) to Trash?",
                    files.len()
                ),
            );
            let response = dialog.run();
            dialog.close();
            if response == gtk::ResponseType::Ok {
                let result = files
                    .iter()
                    .try_for_each(|path| gio::File::for_path(path).trash(gio::Cancellable::NONE));
                state.finish(result);
            }
        });
        for (button, moving) in [(&copy, false), (&cut, true)] {
            let state = Rc::downgrade(&browser);
            let transfer = transfer.clone();
            button.connect_clicked(move |_| {
                let Some(state) = state.upgrade() else {
                    return;
                };
                let paths = state.selected();
                if paths.is_empty() {
                    return;
                }
                state.status.set_text(&trf!(
                    "{} itens selecionados para colar",
                    "{} items selected for pasting",
                    paths.len()
                ));
                transfer.replace(Transfer { paths, moving });
            });
        }
        let state = Rc::downgrade(&browser);
        paste.connect_clicked(move |_| {
            let Some(state) = state.upgrade() else {
                return;
            };
            let current = transfer.borrow().clone();
            if current.paths.is_empty() {
                return;
            }
            let destination = state.folder.borrow().clone();
            let mut errors = Vec::new();
            for path in &current.paths {
                let result = transfer_item(path, &destination, current.moving);
                match result {
                    Ok(()) if current.moving => {
                        transfer.borrow_mut().paths.retain(|item| item != path)
                    }
                    Ok(()) => (),
                    Err(error) => errors.push(format!("{}: {error}", path.display())),
                }
            }
            state.refresh();
            if let Some(error) = errors.first() {
                state.status.set_text(&trf!(
                    "{} falha(s) · {error}",
                    "{} failure(s) · {error}",
                    errors.len()
                ));
            }
        });
        browser.commands.replace(HashMap::from([
            ("back", back),
            ("forward", forward),
            ("up", up),
            ("new-folder", create),
            ("copy", copy),
            ("cut", cut),
            ("paste", paste),
            ("rename", rename),
            ("trash", trash),
        ]));
        commands::register(&browser);
        let state = Rc::downgrade(&browser);
        browser.view.connect_button_press_event(move |view, event| {
            if event.button() != 3 || event.window() != view.bin_window() {
                return glib::Propagation::Proceed;
            }
            let Some(state) = state.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let (x, y) = event.position();
            let row = view
                .path_at_pos(x as i32, y as i32)
                .and_then(|(path, _, _, _)| path);
            let selection = view.selection();
            if let Some(row) = row.as_ref() {
                if !selection.path_is_selected(row) {
                    selection.unselect_all();
                    selection.select_path(row);
                }
            } else {
                selection.unselect_all();
            }
            let selected = state.selected();
            let menu = gtk::Menu::new();
            if let [path] = selected.as_slice() {
                let target = path.clone();
                let owner = Rc::downgrade(&state);
                menu_action(&menu, tr!("Abrir", "Open"), move || {
                    if let Some(owner) = owner.upgrade() {
                        owner.open(&target);
                    }
                });
                let target = path.clone();
                menu_action(&menu, tr!("Copiar caminho", "Copy Path"), move || {
                    gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD)
                        .set_text(&target.to_string_lossy());
                });
                let target = path.clone();
                let owner = Rc::downgrade(&state);
                menu_action(&menu, tr!("Propriedades", "Properties"), move || {
                    if let Some(owner) = owner.upgrade() {
                        owner.properties(&target);
                    }
                });
            }
            let folder_to_pin = match selected.as_slice() {
                [] => Some(state.folder.borrow().clone()),
                [path] if path.is_dir() => Some(path.clone()),
                _ => None,
            };
            if let Some(path) = folder_to_pin {
                let owner = Rc::downgrade(&state);
                menu_action(
                    &menu,
                    tr!(
                        "Fixar pasta no início e na lateral",
                        "Pin Folder to Home and Sidebar"
                    ),
                    move || {
                        if let Err(error) = preferences::pin(&path) {
                            if let Some(owner) = owner.upgrade() {
                                owner.status.set_text(&error.to_string());
                            }
                        }
                    },
                );
            }
            if let [path] = selected.as_slice() {
                if path.is_dir() {
                    let target = path.clone();
                    let owner = Rc::downgrade(&state);
                    menu_action(
                        &menu,
                        tr!("Abrir em nova janela", "Open in New Window"),
                        move || {
                            if let Err(error) = commands::new_window(&target) {
                                if let Some(owner) = owner.upgrade() {
                                    owner.status.set_text(&error.to_string());
                                }
                            }
                        },
                    );
                    let target = path.clone();
                    let owner = Rc::downgrade(&state);
                    menu_action(
                        &menu,
                        tr!("Abrir em nova aba", "Open in New Tab"),
                        move || {
                            if let Some(owner) = owner.upgrade() {
                                owner.open_tab(&target);
                            }
                        },
                    );
                } else {
                    let target = path.clone();
                    let owner = Rc::downgrade(&state);
                    menu_action(&menu, tr!("Abrir com…", "Open With…"), move || {
                        if let Some(owner) = owner.upgrade() {
                            owner.open_with(&target);
                        }
                    });
                }
            }
            if selected.is_empty()
                || selected
                    .as_slice()
                    .first()
                    .is_some_and(|path| selected.len() == 1 && path.is_dir())
            {
                let directory = selected
                    .first()
                    .cloned()
                    .unwrap_or_else(|| state.folder.borrow().clone());
                let owner = Rc::downgrade(&state);
                menu_action(
                    &menu,
                    tr!("Abrir terminal aqui", "Open Terminal Here"),
                    move || {
                        if let Err(error) = commands::terminal(&directory) {
                            if let Some(owner) = owner.upgrade() {
                                owner.status.set_text(&error.to_string());
                            }
                        }
                    },
                );
            }
            let commands = [
                ("new-folder", tr!("Nova pasta", "New Folder")),
                ("paste", tr!("Colar", "Paste")),
                (
                    "empty-file",
                    tr!("Criar arquivo vazio", "Create Empty File"),
                ),
                ("copy", tr!("Copiar", "Copy")),
                ("cut", tr!("Recortar", "Cut")),
                ("rename", tr!("Renomear", "Rename")),
                ("trash", tr!("Mover para a lixeira", "Move to Trash")),
            ];
            for (name, label) in commands {
                let enabled = match name {
                    "new-folder" | "paste" | "empty-file" => true,
                    "rename" => selected.len() == 1,
                    _ => !selected.is_empty(),
                };
                if enabled {
                    let owner = Rc::downgrade(&state);
                    menu_action(&menu, label, move || {
                        if let Some(owner) = owner.upgrade() {
                            owner.dispatch(name);
                        }
                    });
                }
            }
            if !selected.is_empty() {
                let owner = Rc::downgrade(&state);
                menu_action(
                    &menu,
                    tr!("Excluir permanentemente…", "Delete Permanently…"),
                    move || {
                        if let Some(owner) = owner.upgrade() {
                            owner.dispatch("delete");
                        }
                    },
                );
            } else {
                let owner = Rc::downgrade(&state);
                menu_action(&menu, tr!("Recarregar", "Reload"), move || {
                    if let Some(owner) = owner.upgrade() {
                        owner.dispatch("reload");
                    }
                });
            }
            show_menu(&menu, event);
            glib::Propagation::Stop
        });
        (browser, root)
    }

    fn open_tab(&self, path: &Path) {
        if let Some(tabs) = self
            .root
            .upgrade()
            .and_then(|root| root.ancestor(gtk::Notebook::static_type()))
            .and_then(|widget| widget.downcast::<gtk::Notebook>().ok())
        {
            open_tab(&tabs, path, self.transfer.clone());
        }
    }

    fn open(self: &Rc<Self>, path: &Path) {
        if path.is_dir() {
            self.navigate(path, true);
        } else {
            let uri = gio::File::for_path(path).uri();
            if let Err(error) =
                gio::AppInfo::launch_default_for_uri(&uri, gio::AppLaunchContext::NONE)
            {
                self.status.set_text(&error.to_string());
            }
        }
    }

    fn properties(&self, path: &Path) {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) => {
                self.status.set_text(&error.to_string());
                return;
            }
        };
        let (kind, _) = file_display(path, metadata.is_dir(), metadata.is_symlink());
        let message = trf!(
            "{}\n\nTipo  {kind}\nTamanho  {} bytes\nCriação  {}\nModificação  {}",
            "{}\n\nType  {kind}\nSize  {} bytes\nCreated  {}\nModified  {}",
            path.display(),
            metadata.len(),
            display_date(metadata.created().ok()),
            display_date(metadata.modified().ok())
        );
        let dialog = gtk::MessageDialog::new(
            self.window().as_ref(),
            gtk::DialogFlags::MODAL,
            gtk::MessageType::Info,
            gtk::ButtonsType::Close,
            &message,
        );
        dialog.set_title(tr!("Propriedades", "Properties"));
        dialog.run();
        dialog.close();
    }

    fn queue_thumbnails(self: &Rc<Self>) {
        if self.thumbnail_pending.replace(true) {
            return;
        }
        let state = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(state) = state.upgrade() {
                state.thumbnail_pending.set(false);
                if state.load_thumbnails() {
                    state.queue_thumbnails();
                }
            }
        });
    }

    fn load_thumbnails(&self) -> bool {
        let Some((start, end)) = self.view.visible_range() else {
            return false;
        };
        let (Some(first), Some(last)) = (
            start.indices().first().copied(),
            end.indices().first().copied(),
        ) else {
            return false;
        };
        let paths = self.paths.borrow();
        let mut cached = self.thumbnails.borrow_mut();
        let mut attempted = 0;
        for index in first..=last.min(first.saturating_add(119)) {
            if cached.contains(&index) {
                continue;
            }
            let Some(iter) = self.store.iter_nth_child(None, index) else {
                continue;
            };
            if self.store.value(&iter, 2).get::<String>().as_deref() != Ok(tr!("Imagem", "Image")) {
                continue;
            }
            let Some(path) = paths.get(index as usize) else {
                continue;
            };
            if attempted == 2 {
                return true;
            }
            attempted += 1;
            cached.push_back(index);
            if let Some(preview) = image_thumbnail(path) {
                self.store.set_value(&iter, 7, &preview.to_value());
            }
            if cached.len() > 160 {
                if let Some(old) = cached
                    .pop_front()
                    .and_then(|row| self.store.iter_nth_child(None, row))
                {
                    self.store.set_value(
                        &old,
                        7,
                        &Option::<gtk::gdk_pixbuf::Pixbuf>::None.to_value(),
                    );
                }
            }
        }
        false
    }

    fn window(&self) -> Option<gtk::Window> {
        self.root.upgrade()?.toplevel()?.downcast().ok()
    }

    fn ask_name(&self, title: &str, initial: &str) -> Option<OsString> {
        let dialog = gtk::Dialog::with_buttons(
            Some(title),
            self.window().as_ref(),
            gtk::DialogFlags::MODAL,
            &[
                (tr!("Cancelar", "Cancel"), gtk::ResponseType::Cancel),
                (tr!("Salvar", "Save"), gtk::ResponseType::Ok),
            ],
        );
        dialog.set_default_response(gtk::ResponseType::Ok);
        let entry = gtk::Entry::new();
        entry.set_text(initial);
        entry.set_activates_default(true);
        dialog.content_area().add(&entry);
        dialog.show_all();
        let response = dialog.run();
        let name = entry.text();
        dialog.close();
        if response != gtk::ResponseType::Ok {
            return None;
        }
        if name.is_empty() || name.contains('/') || matches!(name.as_str(), "." | "..") {
            self.status.set_text(tr!(
                "Digite um nome válido para o item.",
                "Enter a valid item name."
            ));
            return None;
        }
        Some(OsString::from(name.as_str()))
    }

    fn finish<T, E: std::fmt::Display>(self: &Rc<Self>, result: Result<T, E>) {
        self.refresh();
        if let Err(error) = result {
            self.status.set_text(&error.to_string());
        }
    }

    fn path_at(&self, row: &gtk::TreePath) -> Option<PathBuf> {
        let iter = self.store.iter(row)?;
        let index = self.store.value(&iter, 0).get::<u32>().ok()?;
        self.paths.borrow().get(index as usize).cloned()
    }

    fn selected(&self) -> Vec<PathBuf> {
        self.view
            .selection()
            .selected_rows()
            .0
            .iter()
            .filter_map(|row| self.path_at(row))
            .collect()
    }

    fn navigate(self: &Rc<Self>, path: &Path, remember: bool) -> bool {
        let path = match path
            .canonicalize()
            .and_then(|path| fs::read_dir(&path).map(|_| path))
        {
            Ok(path) => path,
            Err(error) => {
                self.status.set_text(&error.to_string());
                return false;
            }
        };
        let old = self.folder.replace(path.clone());
        if remember && old != path {
            self.history.borrow_mut().push(old);
            self.forward.borrow_mut().clear();
        }
        self.address.set_text(&path.to_string_lossy());
        self.filter.set_text("");
        self.refresh();
        true
    }

    fn refresh(self: &Rc<Self>) {
        self.thumbnails.borrow_mut().clear();
        self.store.clear();
        self.paths.borrow_mut().clear();
        let folder = self.folder.borrow();
        let entries = match fs::read_dir(&*folder) {
            Ok(entries) => entries,
            Err(error) => {
                self.status.set_text(&error.to_string());
                return;
            }
        };
        let filter = self.filter.text().to_lowercase();
        let show_hidden = self.hidden.is_active();
        let mut rows = Vec::new();
        let mut unreadable = 0;
        for entry in entries {
            let Ok(entry) = entry else {
                unreadable += 1;
                continue;
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            let key = name.to_lowercase();
            if (!show_hidden && name.starts_with('.')) || !key.contains(&filter) {
                continue;
            }
            let path = entry.path();
            let metadata = entry.metadata().ok();
            let link = metadata.as_ref().is_some_and(|meta| meta.is_symlink());
            let directory =
                metadata.as_ref().is_some_and(|meta| meta.is_dir()) || (link && path.is_dir());
            let bytes = metadata
                .as_ref()
                .filter(|meta| meta.is_file())
                .map(|meta| meta.len());
            let created = metadata.as_ref().and_then(|meta| meta.created().ok());
            let modified = metadata.as_ref().and_then(|meta| meta.modified().ok());
            let (kind, icon) = file_display(&path, directory, link);
            rows.push((
                !directory, key, name, kind, bytes, path, icon, created, modified,
            ));
        }
        let (property, descending) = self.sort.get();
        rows.sort_unstable_by(|a, b| {
            let order = match property {
                2 => a.3.cmp(b.3),
                3 => optional_order(a.4, b.4, descending),
                5 => optional_order(a.7, b.7, descending),
                6 => optional_order(a.8, b.8, descending),
                _ => a.1.cmp(&b.1),
            };
            let ordered = if descending && matches!(property, 1 | 2) {
                order.reverse()
            } else {
                order
            };
            a.0.cmp(&b.0)
                .then(ordered)
                .then_with(|| (&a.1, &a.5).cmp(&(&b.1, &b.5)))
        });
        let mut paths = self.paths.borrow_mut();
        for (_, _, name, kind, bytes, path, icon, created, modified) in rows {
            let index = paths.len() as u32;
            paths.push(path);
            let size = bytes.map(format_size).unwrap_or_default();
            let created = display_date(created);
            let modified = display_date(modified);
            self.store.insert_with_values(
                None,
                &[
                    (0, &index),
                    (1, &name),
                    (2, &kind),
                    (3, &size),
                    (4, &icon),
                    (5, &created),
                    (6, &modified),
                ],
            );
        }
        self.status.set_text(&if unreadable == 0 {
            trf!("{} itens", "{} items", paths.len())
        } else {
            trf!(
                "{} itens · {unreadable} entradas ilegíveis",
                "{} items · {unreadable} unreadable entries",
                paths.len()
            )
        });
        self.queue_thumbnails();
    }
}

fn transfer_item(source: &Path, directory: &Path, moving: bool) -> Result<(), glib::Error> {
    let reject = |message| glib::Error::new(gio::IOErrorEnum::InvalidArgument, message);
    let name = source
        .file_name()
        .ok_or_else(|| reject(tr!("Nome inválido", "Invalid name")))?;
    if directory.starts_with(source) {
        return Err(reject(tr!(
            "O destino está dentro da origem",
            "The destination is inside the source"
        )));
    }
    let target = directory.join(name);
    if source == target {
        return Err(reject(tr!(
            "Origem e destino são iguais",
            "Source and destination are the same"
        )));
    }
    let source_file = gio::File::for_path(source);
    let target_file = gio::File::for_path(target);
    let flags = gio::FileCopyFlags::NOFOLLOW_SYMLINKS;
    if moving {
        return source_file.move_(&target_file, flags, gio::Cancellable::NONE, None);
    }
    if fs::symlink_metadata(source).is_ok_and(|meta| meta.is_dir()) {
        let staging = directory.join(format!(".alpha-copy-{}", glib::uuid_string_random()));
        let staging_file = gio::File::for_path(&staging);
        staging_file.make_directory(gio::Cancellable::NONE)?;
        let copy = (|| {
            let staged_item = staging.join(name);
            copy_directory(source, &staged_item)?;
            gio::File::for_path(staged_item).move_(
                &target_file,
                gio::FileCopyFlags::NO_FALLBACK_FOR_MOVE | flags,
                gio::Cancellable::NONE,
                None,
            )
        })();
        let cleanup = fs::remove_dir_all(staging);
        return copy.and_then(|()| {
            cleanup.map_err(|error| glib::Error::new(gio::IOErrorEnum::Failed, &error.to_string()))
        });
    }
    source_file.copy(&target_file, flags, gio::Cancellable::NONE, None)
}

fn copy_directory(source: &Path, target: &Path) -> Result<(), glib::Error> {
    let io_error =
        |error: std::io::Error| glib::Error::new(gio::IOErrorEnum::Failed, &error.to_string());
    fs::create_dir(target).map_err(io_error)?;
    for entry in fs::read_dir(source).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let destination = target.join(entry.file_name());
        if entry.file_type().map_err(io_error)?.is_dir() {
            copy_directory(&entry.path(), &destination)?;
        } else {
            gio::File::for_path(entry.path()).copy(
                &gio::File::for_path(destination),
                gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                gio::Cancellable::NONE,
                None,
            )?;
        }
    }
    Ok(())
}

fn open_tab(tabs: &gtk::Notebook, path: &Path, transfer: Rc<RefCell<Transfer>>) {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let tools = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    tools.set_border_width(8);
    tools.style_context().add_class("tab-actions");
    let split = gtk::ToggleButton::with_label(tr!("Painel duplo", "Split View"));
    tools.pack_end(&split, false, false, 0);
    page.pack_start(&tools, false, false, 0);
    let panes = gtk::Paned::new(gtk::Orientation::Horizontal);
    let (first, first_root) = Browser::new(path, transfer.clone());
    panes.pack1(&first_root, true, false);
    page.pack_start(&panes, true, true, 0);
    let label = gtk::Label::new(Some(tr!("Arquivos", "Files")));
    let tab_label = label.clone();
    first.address.connect_changed(move |entry| {
        let text = entry.text();
        let display = Path::new(text.as_str())
            .file_name()
            .map(|name| name.to_string_lossy());
        tab_label.set_text(display.as_deref().unwrap_or("/"));
    });
    label.set_text(
        &path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy(),
    );
    split.connect_toggled(move |button| {
        if button.is_active() {
            let (_, second_root) = Browser::new(&first.folder.borrow(), transfer.clone());
            panes.pack2(&second_root, true, false);
            panes.set_position(panes.allocated_width() / 2);
            second_root.show_all();
        } else if let Some(right) = panes.child2() {
            panes.remove(&right);
        }
    });
    let tab = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let close = icon_button("window-close-symbolic", tr!("Fechar aba", "Close Tab"));
    close.style_context().add_class("tab-close");
    tab.pack_start(&label, true, true, 0);
    tab.pack_end(&close, false, false, 0);
    tab.show_all();
    let notebook = tabs.downgrade();
    let weak_page = page.downgrade();
    close.connect_clicked(move |_| {
        if let (Some(tabs), Some(page)) = (notebook.upgrade(), weak_page.upgrade()) {
            if let Some(index) = tabs.page_num(&page) {
                tabs.remove_page(Some(index));
            }
        }
    });
    let index = tabs.append_page(&page, Some(&tab));
    tabs.set_tab_reorderable(&page, true);
    page.show_all();
    tabs.set_current_page(Some(index));
}

fn main() -> glib::ExitCode {
    let app = gtk::Application::new(
        Some("dev.jcdev.AlphaFileManager"),
        gio::ApplicationFlags::NON_UNIQUE,
    );
    let initial = std::env::args_os()
        .nth(1)
        .map(|argument| {
            if argument.as_encoded_bytes().starts_with(b"file://") {
                gio::File::for_uri(&argument.to_string_lossy())
                    .path()
                    .unwrap_or_default()
            } else {
                PathBuf::from(argument)
            }
        })
        .and_then(|path| path.canonicalize().ok())
        .filter(|path| path.is_dir())
        .unwrap_or_else(glib::home_dir);
    app.connect_activate(move |app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        let window = gtk::ApplicationWindow::new(app);
        window.set_title("Alpha File Manager");
        window.set_default_size(1120, 680);
        let header = gtk::HeaderBar::new();
        header.set_title(Some("Alpha File Manager"));
        header.style_context().add_class("glass-toolbar");
        let toggle_sidebar = gtk::ToggleButton::new();
        toggle_sidebar.add(&gtk::Image::from_icon_name(
            Some("sidebar-show-symbolic"),
            gtk::IconSize::Button,
        ));
        label_control(
            &toggle_sidebar,
            tr!("Mostrar barra lateral", "Show Sidebar"),
        );
        toggle_sidebar.set_active(true);
        header.pack_start(&toggle_sidebar);
        let new_tab = icon_button("list-add-symbolic", tr!("Nova aba", "New Tab"));
        header.pack_end(&new_tab);
        let settings = icon_button("emblem-system-symbolic", tr!("Configurações", "Settings"));
        header.pack_end(&settings);
        let owner = window.downgrade();
        settings.connect_clicked(move |_| {
            if let Some(window) = owner.upgrade() {
                preferences::settings(&window);
            }
        });
        window.set_icon_name(Some(preferences::ICON));
        window.style_context().add_class("alpha");
        preferences::apply_theme(&window, preferences::profile());

        let shell = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        add_background_grain(&shell);
        let rail = gtk::Box::new(gtk::Orientation::Vertical, 8);
        rail.style_context().add_class("rail");
        let loader = gtk::gdk_pixbuf::PixbufLoader::new();
        loader.set_size(38, 38);
        let _ = loader.write(include_bytes!("../assets/alpha-file-manager.svg"));
        let _ = loader.close();
        let brand = gtk::Image::from_pixbuf(loader.pixbuf().as_ref());
        let sidebar_column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sidebar_column.style_context().add_class("glass-sidebar");
        let brand_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        brand_row.style_context().add_class("brand-row");
        brand_row.set_height_request(49);
        brand_row.set_halign(gtk::Align::Center);
        brand_row.pack_start(&brand, false, false, 0);
        sidebar_column.pack_start(&brand_row, false, false, 0);
        let home = location_button("go-home-symbolic", tr!("Início", "Home"), true);
        let files = location_button("folder-symbolic", tr!("Arquivos", "Files"), true);
        rail.pack_start(&home, false, false, 0);
        rail.pack_start(&files, false, false, 0);
        let sidebar = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        sidebar.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        sidebar.set_width_request(172);
        sidebar.add(&rail);
        sidebar_column.pack_start(&sidebar, true, true, 0);
        let sidebar_revealer = gtk::Revealer::new();
        sidebar_revealer.set_transition_type(gtk::RevealerTransitionType::SlideRight);
        sidebar_revealer.set_transition_duration(160);
        sidebar_revealer.add(&sidebar_column);
        sidebar_revealer.set_reveal_child(true);
        shell.pack_start(&sidebar_revealer, false, false, 0);
        let sidebar_control = sidebar_revealer.downgrade();
        toggle_sidebar.connect_toggled(move |toggle| {
            if let Some(sidebar) = sidebar_control.upgrade() {
                sidebar.set_reveal_child(toggle.is_active());
            }
            label_control(
                toggle,
                if toggle.is_active() {
                    tr!("Recolher barra lateral", "Collapse Sidebar")
                } else {
                    tr!("Expandir barra lateral", "Expand Sidebar")
                },
            );
        });
        label_control(
            &toggle_sidebar,
            tr!("Recolher barra lateral", "Collapse Sidebar"),
        );

        let stack = gtk::Stack::new();
        let tabs = gtk::Notebook::new();
        tabs.set_scrollable(true);
        let transfer = Rc::new(RefCell::new(Transfer::default()));
        let navigation = Navigation {
            stack: stack.downgrade(),
            tabs: tabs.downgrade(),
            transfer: transfer.clone(),
        };
        commands::install(&window, app, &header, &navigation, &toggle_sidebar);
        add_sidebar_locations(&rail, &navigation);
        let dashboard = dashboard(&navigation);
        stack.add_named(&dashboard, "home");
        stack.add_named(&tabs, "files");
        let workspace = gtk::Box::new(gtk::Orientation::Vertical, 0);
        workspace.pack_start(&header, false, false, 0);
        workspace.pack_start(&stack, true, true, 0);
        shell.pack_start(&workspace, true, true, 0);
        window.add(&shell);

        let navigation = stack.downgrade();
        home.connect_clicked(move |_| {
            if let Some(stack) = navigation.upgrade() {
                stack.set_visible_child_name("home");
            }
        });
        let navigation = stack.downgrade();
        let notebook = tabs.downgrade();
        let shared = transfer.clone();
        files.connect_clicked(move |_| {
            if let (Some(stack), Some(tabs)) = (navigation.upgrade(), notebook.upgrade()) {
                if tabs.n_pages() == 0 {
                    open_tab(&tabs, &glib::home_dir(), shared.clone());
                }
                stack.set_visible_child_name("files");
            }
        });
        let navigation = stack.downgrade();
        let notebook = tabs.downgrade();
        let shared = transfer.clone();
        new_tab.connect_clicked(move |_| {
            if let (Some(stack), Some(tabs)) = (navigation.upgrade(), notebook.upgrade()) {
                open_tab(&tabs, &glib::home_dir(), shared.clone());
                stack.set_visible_child_name("files");
            }
        });
        let home_control = home.downgrade();
        let files_control = files.downgrade();
        let update_rail = move |stack: &gtk::Stack| {
            let active = stack.visible_child_name();
            for (name, control) in [("home", &home_control), ("files", &files_control)] {
                if let Some(button) = control.upgrade() {
                    if active.as_deref() == Some(name) {
                        button.style_context().add_class("current");
                    } else {
                        button.style_context().remove_class("current");
                    }
                }
            }
        };
        stack.connect_visible_child_name_notify(update_rail.clone());
        window.show_all();
        if std::env::args_os().len() > 1 {
            open_tab(&tabs, &initial, transfer);
            stack.set_visible_child_name("files");
        } else {
            stack.set_visible_child_name("home");
        }
        update_rail(&stack);
    });
    app.run_with_args(&["alpha-file-manager"])
}

fn add_background_grain(shell: &gtk::Box) {
    use gtk::gdk::prelude::GdkContextExt;
    let loader = gtk::gdk_pixbuf::PixbufLoader::new();
    if loader.write(include_bytes!("../assets/grain.png")).is_err() || loader.close().is_err() {
        return;
    }
    if let Some(grain) = loader.pixbuf() {
        shell.connect_draw(move |_, cairo| {
            if cairo.save().is_ok() {
                cairo.set_source_pixbuf(&grain, 0.0, 0.0);
                cairo.source().set_extend(gtk::cairo::Extend::Repeat);
                let _ = cairo.paint_with_alpha(0.045);
                let _ = cairo.restore();
            }
            glib::Propagation::Proceed
        });
    }
}

fn label_control(widget: &impl IsA<gtk::Widget>, label: &str) {
    widget.set_tooltip_text(Some(label));
    if let Some(accessible) = widget.accessible() {
        accessible.set_name(label);
    }
}

fn icon_button(icon: &str, label: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(Some(icon), gtk::IconSize::Button);
    label_control(&button, label);
    button
}

fn file_display(path: &Path, directory: bool, link: bool) -> (&'static str, &'static str) {
    if link {
        return ("Link", "emblem-symbolic-link");
    }
    if directory {
        return (tr!("Pasta", "Folder"), "folder");
    }

    let extension = path
        .extension()
        .and_then(|part| part.to_str())
        .unwrap_or("");
    match extension.to_ascii_lowercase().as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "avif" | "bmp" | "tif" | "ico" => {
            (tr!("Imagem", "Image"), "image-x-generic")
        }
        "mp4" | "mkv" | "webm" | "avi" | "mov" | "m4v" => {
            (tr!("Vídeo", "Video"), "video-x-generic")
        }
        "mp3" | "flac" | "wav" | "ogg" | "opus" | "m4a" | "aac" => {
            (tr!("Áudio", "Audio"), "audio-x-generic")
        }
        "pdf" => ("PDF", "application-pdf"),
        "zip" | "tar" | "gz" | "xz" | "bz2" | "7z" | "rar" | "zst" => {
            (tr!("Compactado", "Archive"), "package-x-generic")
        }
        "rs" | "py" | "js" | "ts" | "cpp" | "c" | "h" | "go" | "java" | "sh" | "css" | "html"
        | "json" | "toml" | "yaml" | "yml" | "xml" => {
            (tr!("Código", "Source Code"), "text-x-script")
        }
        "csv" | "ods" | "xls" | "xlsx" => (tr!("Planilha", "Spreadsheet"), "x-office-spreadsheet"),
        "odp" | "ppt" | "pptx" => (tr!("Apresentação", "Presentation"), "x-office-presentation"),
        "txt" | "md" | "log" | "odt" | "rtf" | "doc" | "docx" => {
            (tr!("Documento", "Document"), "x-office-document")
        }
        "deb" | "rpm" | "appimage" => {
            (tr!("Aplicativo", "Application"), "application-x-executable")
        }
        _ => (tr!("Arquivo", "File"), "text-x-generic"),
    }
}

#[derive(Clone)]
struct Navigation {
    stack: glib::WeakRef<gtk::Stack>,
    tabs: glib::WeakRef<gtk::Notebook>,
    transfer: Rc<RefCell<Transfer>>,
}

impl Navigation {
    fn open(&self, path: &Path) {
        if let (Some(stack), Some(tabs)) = (self.stack.upgrade(), self.tabs.upgrade()) {
            open_tab(&tabs, path, self.transfer.clone());
            stack.set_visible_child_name("files");
        }
    }
}

fn standard_locations() -> Vec<(&'static str, &'static str, PathBuf)> {
    use glib::UserDirectory::*;
    let mut folders = vec![(
        tr!("Pasta pessoal", "Home Folder"),
        "user-home",
        glib::home_dir(),
    )];
    for (name, icon, directory) in [
        (tr!("Área de trabalho", "Desktop"), "user-desktop", Desktop),
        ("Downloads", "folder-download", Downloads),
        (
            tr!("Documentos", "Documents"),
            "folder-documents",
            Documents,
        ),
        (tr!("Imagens", "Images"), "folder-pictures", Pictures),
        (tr!("Vídeos", "Videos"), "folder-videos", Videos),
        (tr!("Música", "Music"), "folder-music", Music),
    ] {
        if let Some(path) = glib::user_special_dir(directory).filter(|path| path.is_dir()) {
            folders.push((name, icon, path));
        }
    }
    folders
}

fn location_button(icon: &str, name: &str, compact: bool) -> gtk::Button {
    let button = gtk::Button::new();
    button
        .style_context()
        .add_class(if compact { "side-location" } else { "location" });
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 9);
    let size = if compact {
        gtk::IconSize::Button
    } else {
        gtk::IconSize::LargeToolbar
    };
    row.add(&gtk::Image::from_icon_name(Some(icon), size));
    let label = gtk::Label::new(Some(name));
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_max_width_chars(if compact { 16 } else { 24 });
    label.set_xalign(0.0);
    row.pack_start(&label, true, true, 0);
    button.add(&row);
    label_control(&button, name);
    button
}

fn location(
    navigation: &Navigation,
    icon: &str,
    name: &str,
    path: PathBuf,
    compact: bool,
    pinned: bool,
) -> gtk::Button {
    let button = location_button(icon, name, compact);
    button.set_tooltip_text(Some(&path.to_string_lossy()));
    let target = path.clone();
    let open = navigation.clone();
    button.connect_clicked(move |_| open.open(&target));
    if pinned {
        button.connect_button_press_event(move |button, event| {
            if event.button() != 3 {
                return glib::Propagation::Proceed;
            }
            let menu = gtk::Menu::new();
            let path = path.clone();
            let parent = button.downgrade();
            menu_action(
                &menu,
                tr!("Remover dos atalhos", "Remove from Shortcuts"),
                move || {
                    if let Err(error) = preferences::unpin(&path) {
                        if let Some(parent) = parent.upgrade() {
                            show_error(&parent, &error.to_string());
                        }
                    }
                },
            );
            show_menu(&menu, event);
            glib::Propagation::Stop
        });
    }
    button
}

fn pinned_locations(container: &impl IsA<gtk::Container>, navigation: &Navigation, compact: bool) {
    let weak = container.as_ref().downgrade();
    let navigation = navigation.clone();
    let rebuild = move || {
        let Some(container) = weak.upgrade() else {
            return;
        };
        for child in container.children() {
            container.remove(&child);
        }
        let paths = preferences::pinned_paths();
        let has_pins = !paths.is_empty();
        for path in paths {
            let title = path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned();
            container.add(&location(
                &navigation,
                "folder-bookmark",
                &title,
                path,
                compact,
                true,
            ));
        }
        container.set_visible(has_pins);
        for child in container.children() {
            child.show_all();
        }
    };
    rebuild();
    preferences::PINS.with(|model| {
        let inserted = rebuild.clone();
        model.connect_row_inserted(move |_, _, _| inserted());
        model.connect_row_deleted(move |_, _| rebuild());
    });
}

fn section(name: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(name));
    label.set_xalign(0.0);
    label.style_context().add_class("section-title");
    label
}

fn add_sidebar_locations(rail: &gtk::Box, navigation: &Navigation) {
    rail.pack_start(&section(tr!("Pastas", "Folders")), false, false, 4);
    for (name, icon, path) in standard_locations().into_iter().chain([(
        tr!("Sistema", "System"),
        "drive-harddisk",
        PathBuf::from("/"),
    )]) {
        rail.pack_start(
            &location(navigation, icon, name, path, true, false),
            false,
            false,
            0,
        );
    }
    rail.pack_start(&section(tr!("Fixados", "Pinned")), false, false, 4);
    let pins = gtk::Box::new(gtk::Orientation::Vertical, 5);
    rail.pack_start(&pins, false, false, 0);
    pinned_locations(&pins, navigation, true);
}

fn card_grid() -> gtk::FlowBox {
    let cards = gtk::FlowBox::new();
    cards.set_selection_mode(gtk::SelectionMode::None);
    cards.set_homogeneous(true);
    cards.set_max_children_per_line(4);
    cards.set_row_spacing(10);
    cards.set_column_spacing(10);
    cards
}

fn dashboard(navigation: &Navigation) -> gtk::ScrolledWindow {
    let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
    content.set_border_width(24);
    content.pack_start(&banner::widget(), false, false, 0);

    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    heading.pack_start(&section(tr!("Fixados", "Pinned")), true, true, 0);
    let add = gtk::Button::with_label(tr!("Adicionar pasta", "Add Folder"));
    heading.pack_end(&add, false, false, 0);
    content.pack_start(&heading, false, false, 0);
    add.connect_clicked(|button| {
        let parent = button
            .toplevel()
            .and_then(|widget| widget.downcast::<gtk::Window>().ok());
        let chooser = gtk::FileChooserDialog::with_buttons(
            Some(tr!("Fixar uma pasta", "Pin a Folder")),
            parent.as_ref(),
            gtk::FileChooserAction::SelectFolder,
            &[
                (tr!("Cancelar", "Cancel"), gtk::ResponseType::Cancel),
                (tr!("Fixar", "Pin"), gtk::ResponseType::Accept),
            ],
        );
        let chosen = if chooser.run() == gtk::ResponseType::Accept {
            chooser.filename()
        } else {
            None
        };
        chooser.close();
        if let Some(path) = chosen {
            if let Err(error) = preferences::pin(&path) {
                show_error(button, &error.to_string());
            }
        }
    });
    let pinned = card_grid();
    pinned.set_no_show_all(true);
    content.pack_start(&pinned, false, false, 0);
    pinned_locations(&pinned, navigation, false);

    for (heading, locations) in [
        (
            tr!("Pastas pessoais", "Personal Folders"),
            standard_locations(),
        ),
        (
            tr!("Sistema", "System"),
            vec![(
                tr!("Sistema de arquivos", "File System"),
                "drive-harddisk",
                PathBuf::from("/"),
            )],
        ),
    ] {
        content.pack_start(&section(heading), false, false, 0);
        let cards = card_grid();
        for (name, icon, path) in locations {
            cards.add(&location(navigation, icon, name, path, false, false));
        }
        content.pack_start(&cards, false, false, 0);
    }
    scroll.add(&content);
    scroll
}

fn show_error(widget: &impl IsA<gtk::Widget>, message: &str) {
    let parent = widget
        .toplevel()
        .and_then(|widget| widget.downcast::<gtk::Window>().ok());
    let dialog = gtk::MessageDialog::new(
        parent.as_ref(),
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Error,
        gtk::ButtonsType::Close,
        message,
    );
    dialog.run();
    dialog.close();
}

fn menu_action(menu: &gtk::Menu, label: &str, action: impl Fn() + 'static) -> gtk::MenuItem {
    let item = gtk::MenuItem::with_label(label);
    item.connect_activate(move |_| action());
    menu.append(&item);
    item
}

fn show_menu(menu: &gtk::Menu, event: &gdk::EventButton) {
    menu.style_context().add_class("alpha");
    menu.show_all();
    // Keep the transient menu alive until GTK dismisses the popup.
    let active = RefCell::new(Some(menu.clone()));
    menu.connect_hide(move |_| {
        active.borrow_mut().take();
    });
    if let Some(window) = event.window() {
        menu.popup_at_rect(
            &window,
            &gdk::Rectangle::new(event.position().0 as i32, event.position().1 as i32, 1, 1),
            gdk::Gravity::SouthEast,
            gdk::Gravity::NorthWest,
            Some(event),
        );
    }
}

fn optional_order<T: Ord>(
    left: Option<T>,
    right: Option<T>,
    descending: bool,
) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) if descending => right.cmp(&left),
        (Some(left), Some(right)) => left.cmp(&right),
        (left, right) => right.is_some().cmp(&left.is_some()),
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / 1048576.0)
    } else {
        format!("{:.1} GiB", bytes as f64 / 1073741824.0)
    }
}

fn display_date(time: Option<SystemTime>) -> String {
    time.and_then(|time| {
        let seconds = match time.duration_since(UNIX_EPOCH) {
            Ok(value) => i64::try_from(value.as_secs()).ok()?,
            Err(value) => -i64::try_from(value.duration().as_secs()).ok()?,
        };
        glib::DateTime::from_unix_local(seconds)
            .ok()?
            .format("%d/%m/%Y")
            .ok()
    })
    .map_or_else(|| "—".to_owned(), |date| date.to_string())
}

fn image_thumbnail(path: &Path) -> Option<gtk::gdk_pixbuf::Pixbuf> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > 12 * 1024 * 1024 {
        return None;
    }
    let (_, width, height) = gtk::gdk_pixbuf::Pixbuf::file_info(path)?;
    if width <= 0 || height <= 0 || i64::from(width) * i64::from(height) > 20_000_000 {
        return None;
    }
    gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, 32, 32, true).ok()
}
