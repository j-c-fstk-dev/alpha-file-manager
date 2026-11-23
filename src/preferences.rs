use gtk::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::RefCell,
    fs, io,
    path::{Path, PathBuf},
};

pub const DESKTOP_ID: &str = "dev.jcdev.AlphaFileManager.desktop";
pub const ICON: &str = "dev.jcdev.AlphaFileManager";
fn profiles() -> [(&'static str, &'static str, &'static str, &'static str); 4] {
    [
        (
            tr!("Verde fósforo", "Phosphor Green"),
            "#8df4ae",
            "#244936",
            "#121d19",
        ),
        (tr!("Âmbar", "Amber"), "#ffc77a", "#584023", "#211a15"),
        (tr!("Ciano", "Cyan"), "#80edff", "#234953", "#101d23"),
        (tr!("Violeta", "Violet"), "#d6baff", "#4a385f", "#1d1727"),
    ]
}

fn directory() -> PathBuf {
    glib::user_config_dir().join("alpha-file-manager")
}

thread_local! {
    pub static PINS: gtk::ListStore = {
        let model = gtk::ListStore::new(&[String::static_type()]);
        for uri in fs::read_to_string(directory().join("locations")).unwrap_or_default().lines() {
            if gio::File::for_uri(uri).path().is_some() {
                model.insert_with_values(None, &[(0, &uri)]);
            }
        }
        model
    };
    static THEME: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

pub fn pinned_paths() -> Vec<PathBuf> {
    PINS.with(|model| {
        let mut paths = Vec::new();
        model.foreach(|model, _, iter| {
            if let Ok(uri) = model.value(iter, 0).get::<String>() {
                paths.extend(gio::File::for_uri(&uri).path());
            }
            false
        });
        paths
    })
}

fn persist_paths(paths: &[PathBuf]) -> io::Result<()> {
    let text: String = paths
        .iter()
        .map(|path| format!("{}\n", gio::File::for_path(path).uri()))
        .collect();
    write_setting("locations", &text)
}

pub fn pin(path: &Path) -> io::Result<()> {
    let path = path.canonicalize()?;
    fs::read_dir(&path)?;
    let mut paths = pinned_paths();
    if paths.contains(&path) {
        return Ok(());
    }
    paths.push(path.clone());
    persist_paths(&paths)?;
    let uri = gio::File::for_path(path).uri();
    PINS.with(|model| {
        model.insert_with_values(None, &[(0, &uri.as_str())]);
    });
    Ok(())
}

pub fn unpin(path: &Path) -> io::Result<()> {
    let mut paths = pinned_paths();
    paths.retain(|candidate| candidate != path);
    persist_paths(&paths)?;
    let uri = gio::File::for_path(path).uri();
    PINS.with(|model| {
        if let Some(iter) = model.iter_first() {
            loop {
                if model.value(&iter, 0).get::<String>().as_deref() == Ok(uri.as_str()) {
                    model.remove(&iter);
                    break;
                }
                if !model.iter_next(&iter) {
                    break;
                }
            }
        }
    });
    Ok(())
}

fn write_setting(name: &str, content: &str) -> io::Result<()> {
    let dir = directory();
    fs::create_dir_all(&dir)?;
    let temporary = dir.join(format!("{name}.{}", std::process::id()));
    fs::write(&temporary, content)?;
    fs::rename(temporary, dir.join(name))
}

pub fn profile() -> u32 {
    fs::read_to_string(directory().join("profile"))
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| (*value as usize) < profiles().len())
        .unwrap_or_default()
}

fn transparency() -> bool {
    fs::read_to_string(directory().join("transparency")).is_ok_and(|value| value.trim() == "true")
}

fn opacity() -> u8 {
    fs::read_to_string(directory().join("opacity"))
        .ok()
        .and_then(|value| value.trim().parse::<u8>().ok())
        .filter(|value| (60..=100).contains(value))
        .unwrap_or(94)
}

fn apply_transparency(window: &gtk::ApplicationWindow, enabled: bool, opacity: u8) {
    let style = window.style_context();
    if enabled {
        style.add_class("translucent");
    } else {
        style.remove_class("translucent");
    }
    let composited =
        gtk::prelude::GtkWindowExt::screen(window).is_some_and(|screen| screen.is_composited());
    window.set_opacity(if enabled && composited {
        f64::from(opacity) / 100.0
    } else {
        1.0
    });
}

pub fn apply_theme(window: &gtk::ApplicationWindow, profile: u32) {
    apply_transparency(window, transparency(), opacity());
    let Some(screen) = gtk::prelude::GtkWindowExt::screen(window) else {
        return;
    };
    let (_, accent, selection, background) = profiles()[profile as usize];
    let data = format!(
        "@define-color phosphor {accent};\n@define-color phosphor_selection {selection};\n@define-color phosphor_bg {background};\n{}",
        include_str!("alpha.css")
    );
    let provider = gtk::CssProvider::new();
    if let Err(error) = provider.load_from_data(data.as_bytes()) {
        eprintln!("{}", trf!("Tema Alpha: {error}", "Alpha theme: {error}"));
        return;
    }
    THEME.with(|current| {
        if let Some(previous) = current.replace(Some(provider.clone())) {
            gtk::StyleContext::remove_provider_for_screen(&screen, &previous);
        }
    });
    gtk::StyleContext::add_provider_for_screen(
        &screen,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn runs_xfce() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .any(|desktop| desktop.eq_ignore_ascii_case("xfce"))
}

fn xfce_preferences() -> PathBuf {
    glib::user_config_dir().join("xfce4/helpers.rc")
}

fn xfce_has_alpha() -> bool {
    fs::read_to_string(xfce_preferences())
        .unwrap_or_default()
        .lines()
        .any(|line| line.trim() == format!("FileManager={ICON}"))
}

fn set_xfce_default() -> io::Result<()> {
    if !glib::user_data_dir()
        .join("xfce4/helpers")
        .join(DESKTOP_ID)
        .is_file()
    {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            tr!(
                "Registro do Alpha no Xfce ausente.",
                "Alpha is not registered with Xfce."
            ),
        ));
    }
    let file = xfce_preferences();
    let content = match fs::read_to_string(&file) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let mut updated = String::new();
    for line in content.lines() {
        if !line
            .split_once('=')
            .is_some_and(|(key, _)| key.trim() == "FileManager")
        {
            updated.push_str(line);
            updated.push('\n');
        }
    }
    updated.push_str(&format!("FileManager={ICON}\n"));
    fs::create_dir_all(file.parent().unwrap())?;
    let temporary = file.with_extension(format!("rc.{}", std::process::id()));
    fs::write(&temporary, updated)?;
    fs::rename(temporary, file)
}

pub fn settings(window: &gtk::ApplicationWindow) {
    let dialog = gtk::Dialog::with_buttons(
        Some(tr!("Configurações do Alpha", "Alpha Settings")),
        Some(window),
        gtk::DialogFlags::MODAL,
        &[(tr!("Fechar", "Close"), gtk::ResponseType::Close)],
    );
    dialog.style_context().add_class("alpha");
    dialog.set_default_size(410, 200);
    let content = dialog.content_area();
    content.set_border_width(18);
    content.set_spacing(12);
    let colors = gtk::ComboBoxText::new();
    for (name, ..) in profiles() {
        colors.append_text(name);
    }
    colors.set_active(Some(profile()));
    let label = gtk::Label::new(Some(tr!("Perfil de cores", "Color Profile")));
    label.set_xalign(0.0);
    label.set_mnemonic_widget(Some(&colors));
    content.add(&label);
    content.add(&colors);
    let glass = gtk::CheckButton::with_label(tr!("Ativar transparência", "Enable Transparency"));
    glass.set_active(transparency());
    content.add(&glass);
    let opacity_label = gtk::Label::new(Some(tr!("Opacidade da janela", "Window Opacity")));
    opacity_label.set_xalign(0.0);
    let slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 60.0, 100.0, 1.0);
    slider.set_value(f64::from(opacity()));
    slider.set_digits(0);
    slider.connect_format_value(|_, value| format!("{value:.0}%"));
    opacity_label.set_mnemonic_widget(Some(&slider));
    let opacity_row = gtk::Box::new(gtk::Orientation::Vertical, 4);
    opacity_row.add(&opacity_label);
    opacity_row.add(&slider);
    opacity_row.set_sensitive(glass.is_active());
    content.add(&opacity_row);
    let set_default = gtk::Button::with_label(tr!(
        "Usar o Alpha como gerenciador padrão",
        "Use Alpha as the Default File Manager"
    ));
    let status = gtk::Label::new(None);
    status.set_xalign(0.0);
    status.set_line_wrap(true);
    let update_status = |label: &gtk::Label| {
        let active = gio::AppInfo::default_for_type("inode/directory", false);
        let is_default = active.as_ref().and_then(|app| app.id()).as_deref() == Some(DESKTOP_ID);
        let name = active
            .map(|info| info.display_name().to_string())
            .unwrap_or_else(|| tr!("Nenhum", "None").into());
        let xfce_pending = runs_xfce() && !xfce_has_alpha();
        let suffix = if xfce_pending {
            tr!(
                "\nO atalho de arquivos do Xfce ainda usa a preferência atual do desktop.",
                "\nThe Xfce file manager shortcut still uses the current desktop preference."
            )
        } else {
            ""
        };
        label.set_text(&trf!(
            "Aplicativo para abrir pastas — {name}{suffix}",
            "Application for opening folders — {name}{suffix}"
        ));
        is_default && !xfce_pending
    };
    set_default.set_sensitive(!update_status(&status));
    content.add(&set_default);
    content.add(&status);
    let weak_window = window.downgrade();
    let result = status.clone();
    colors.connect_changed(move |combo| {
        if let (Some(window), Some(selected)) = (weak_window.upgrade(), combo.active()) {
            match write_setting("profile", &selected.to_string()) {
                Ok(()) => apply_theme(&window, selected),
                Err(error) => result.set_text(&error.to_string()),
            }
        }
    });
    let weak_window = window.downgrade();
    let result = status.clone();
    glass.connect_toggled(move |toggle| {
        if let Some(window) = weak_window.upgrade() {
            let enabled = toggle.is_active();
            opacity_row.set_sensitive(enabled);
            match write_setting("transparency", if enabled { "true" } else { "false" }) {
                Ok(()) => apply_transparency(&window, enabled, opacity()),
                Err(error) => result.set_text(&error.to_string()),
            }
        }
    });
    let weak_window = window.downgrade();
    let result = status.clone();
    slider.connect_value_changed(move |slider| {
        let value = slider.value().round() as u8;
        match write_setting("opacity", &value.to_string()) {
            Ok(()) => {
                if let Some(window) = weak_window.upgrade() {
                    apply_transparency(&window, true, value);
                }
            }
            Err(error) => result.set_text(&error.to_string()),
        }
    });
    set_default.connect_clicked(move |button| {
        let Some(app) = gio::DesktopAppInfo::new(DESKTOP_ID) else {
            status.set_text(tr!(
                "O lançador do Alpha precisa estar instalado para escolher o aplicativo padrão.",
                "Install the Alpha launcher before choosing it as the default application."
            ));
            return;
        };
        match app.set_as_default_for_type("inode/directory") {
            Ok(()) => {
                if runs_xfce() {
                    if let Err(error) = set_xfce_default() {
                        status.set_text(&trf!(
                            "Associação de pastas salva. Configuração do Xfce: {error}",
                            "Folder association saved. Xfce setup: {error}"
                        ));
                        return;
                    }
                }
                button.set_sensitive(!update_status(&status));
            }
            Err(error) => status.set_text(&error.to_string()),
        }
    });
    dialog.show_all();
    dialog.run();
    dialog.close();
}
