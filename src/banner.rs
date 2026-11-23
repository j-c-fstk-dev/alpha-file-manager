use gtk::gdk::prelude::GdkContextExt;
use gtk::gdk_pixbuf::{Pixbuf, PixbufLoader};
use gtk::glib;
use gtk::prelude::*;
use std::{cell::RefCell, fs, path::PathBuf, rc::Rc};

fn custom_image() -> PathBuf {
    glib::user_config_dir().join("alpha-file-manager/banner.jpg")
}

fn default_image() -> Option<Pixbuf> {
    let loader = PixbufLoader::new();
    loader.write(include_bytes!("../assets/banner.jpg")).ok()?;
    loader.close().ok()?;
    loader.pixbuf()
}

fn paint(area: &gtk::DrawingArea, cairo: &gtk::cairo::Context, image: &Pixbuf) {
    let width = f64::from(area.allocated_width());
    let height = f64::from(area.allocated_height());
    let iw = f64::from(image.width());
    let ih = f64::from(image.height());
    let factor = (width / iw).max(height / ih);
    if cairo.save().is_ok() {
        let radius = 18.0_f64.min(width / 2.0).min(height / 2.0);
        let quarter = std::f64::consts::FRAC_PI_2;
        cairo.new_sub_path();
        cairo.arc(width - radius, radius, radius, -quarter, 0.0);
        cairo.arc(width - radius, height - radius, radius, 0.0, quarter);
        cairo.arc(radius, height - radius, radius, quarter, quarter * 2.0);
        cairo.arc(radius, radius, radius, quarter * 2.0, quarter * 3.0);
        cairo.close_path();
        cairo.clip();
        if cairo.save().is_ok() {
            cairo.translate((width - iw * factor) / 2.0, (height - ih * factor) / 2.0);
            cairo.scale(factor, factor);
            cairo.set_source_pixbuf(image, 0.0, 0.0);
            let _ = cairo.paint();
            let _ = cairo.restore();
        }
        let shade = gtk::cairo::LinearGradient::new(0.0, 0.0, width, height);
        shade.add_color_stop_rgba(0.0, 0.02, 0.04, 0.04, 0.70);
        shade.add_color_stop_rgba(0.72, 0.02, 0.04, 0.04, 0.27);
        shade.add_color_stop_rgba(1.0, 0.02, 0.04, 0.04, 0.48);
        if cairo.set_source(&shade).is_ok() {
            let _ = cairo.paint();
        }
        let _ = cairo.restore();
    }
}

pub fn widget() -> gtk::Overlay {
    let overlay = gtk::Overlay::new();
    overlay.style_context().add_class("hero");
    let canvas = gtk::DrawingArea::new();
    canvas.set_height_request(254);
    let image = Rc::new(RefCell::new(
        Pixbuf::from_file(custom_image())
            .ok()
            .or_else(default_image),
    ));
    let current = image.clone();
    canvas.connect_draw(move |area, cairo| {
        if let Some(image) = current.borrow().as_ref() {
            paint(area, cairo, image);
        }
        glib::Propagation::Proceed
    });
    overlay.add(&canvas);

    let title = gtk::Label::new(Some("Alpha File Manager"));
    title.style_context().add_class("home-title");
    title.set_halign(gtk::Align::Start);
    title.set_valign(gtk::Align::End);
    title.set_margin_start(25);
    title.set_margin_bottom(24);
    overlay.add_overlay(&title);
    let change = crate::icon_button("image-x-generic-symbolic", "Alterar imagem do banner");
    change.style_context().add_class("banner-control");
    change.set_halign(gtk::Align::End);
    change.set_valign(gtk::Align::Start);
    change.set_margin_top(14);
    change.set_margin_end(14);
    overlay.add_overlay(&change);
    change.connect_clicked(move |button| {
        let parent = button
            .toplevel()
            .and_then(|widget| widget.downcast::<gtk::Window>().ok());
        let chooser = gtk::FileChooserDialog::with_buttons(
            Some("Imagem do banner"),
            parent.as_ref(),
            gtk::FileChooserAction::Open,
            &[
                ("Restaurar padrão", gtk::ResponseType::Other(1)),
                ("Cancelar", gtk::ResponseType::Cancel),
                ("Usar imagem", gtk::ResponseType::Accept),
            ],
        );
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Imagens"));
        filter.add_pixbuf_formats();
        chooser.add_filter(filter);
        let response = chooser.run();
        let file = chooser.filename();
        chooser.close();
        let result = match response {
            gtk::ResponseType::Accept => file.map(import_image),
            gtk::ResponseType::Other(1) => Some(reset_image()),
            _ => None,
        };
        match result {
            Some(Ok(next)) => {
                image.replace(next);
                canvas.queue_draw();
            }
            Some(Err(error)) => crate::show_error(button, &error),
            None => {}
        }
    });
    overlay
}

fn import_image(path: PathBuf) -> Result<Option<Pixbuf>, String> {
    let meta = fs::metadata(&path).map_err(|error| error.to_string())?;
    if !meta.is_file() || meta.len() > 30 * 1024 * 1024 {
        return Err("Escolha um arquivo de imagem de até 30 MiB.".into());
    }
    let (_, width, height) = Pixbuf::file_info(&path).ok_or("Formato de imagem inválido.")?;
    if width <= 0 || height <= 0 || i64::from(width) * i64::from(height) > 35_000_000 {
        return Err("Escolha uma imagem de até 35 megapixels.".into());
    }
    let image =
        Pixbuf::from_file_at_scale(&path, 1920, 1080, true).map_err(|error| error.to_string())?;
    let image = image.apply_embedded_orientation().unwrap_or(image);
    let destination = custom_image();
    let directory = destination.parent().unwrap();
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let temp = directory.join(format!("banner-{}.jpg", glib::uuid_string_random()));
    let result = image
        .savev(&temp, "jpeg", &[("quality", "88")])
        .map_err(|error| error.to_string())
        .and_then(|()| fs::rename(&temp, &destination).map_err(|error| error.to_string()));
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result.map(|()| Some(image))
}

fn reset_image() -> Result<Option<Pixbuf>, String> {
    match fs::remove_file(custom_image()) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    Ok(default_image())
}
