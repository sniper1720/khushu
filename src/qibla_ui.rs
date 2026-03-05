use crate::config::AppConfig;
use crate::i18n::tr;
use crate::qibla::{CompassManager, calculate_qibla_bearing};
use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;

pub struct QiblaPage {
    pub container: gtk::Box,
    pub refresh: Rc<dyn Fn()>,
}

pub fn create_qibla_page(
    config: Rc<RefCell<AppConfig>>,
    compass_manager: Rc<CompassManager>,
) -> QiblaPage {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 24);
    container.set_valign(gtk::Align::Center);
    container.set_halign(gtk::Align::Center);
    container.set_margin_top(48);
    container.set_margin_bottom(48);

    let drawing_area = gtk::DrawingArea::builder()
        .content_width(300)
        .content_height(300)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();

    let lang_val = config.borrow().language.clone();
    let bearing_label = gtk::Label::builder()
        .label(tr("Calculating...", &lang_val))
        .css_classes(["title-1"])
        .build();

    let status_label = gtk::Label::builder()
        .label("")
        .css_classes(["dim-label"])
        .build();

    container.append(&drawing_area);
    container.append(&bearing_label);
    container.append(&status_label);

    let current_rotation = Rc::new(RefCell::new(0.0));
    let bearing = Rc::new(RefCell::new(0.0));

    let rotation_draw = current_rotation.clone();
    let bearing_draw = bearing.clone();

    let qibla_icon = gtk::gdk_pixbuf::Pixbuf::from_resource_at_scale(
        "/io/github/sniper1720/khushu/icons/hicolor/scalable/actions/qibla-symbolic.svg",
        32,
        32,
        true,
    )
    .ok();

    let config_draw = config.clone();
    drawing_area.set_draw_func(move |_, cr, width, height| {
        let cx = width as f64 / 2.0;
        let cy = height as f64 / 2.0;

        let radius = cx.min(cy) - 60.0;

        cr.set_source_rgba(0.5, 0.5, 0.5, 0.3);
        cr.set_line_width(4.0);
        cr.arc(cx, cy, radius, 0.0, 2.0 * PI);
        cr.stroke().expect("Cairo error");

        cr.set_source_rgb(0.8, 0.8, 0.8);
        let lang = config_draw.borrow().language.clone();
        let cardinals = [
            tr("N", &lang),
            tr("E", &lang),
            tr("S", &lang),
            tr("W", &lang),
        ];

        let pango_ctx = pangocairo::functions::create_context(cr);
        let layout = gtk::pango::Layout::new(&pango_ctx);
        let mut font_desc = gtk::pango::FontDescription::new();
        font_desc.set_family("Amiri, Amiri-Regular");
        font_desc.set_weight(gtk::pango::Weight::Bold);
        font_desc.set_size(12 * gtk::pango::SCALE);
        layout.set_font_description(Some(&font_desc));

        for (i, dir) in cardinals.iter().enumerate() {
            layout.set_text(dir);
            let (ink_rect, _) = layout.extents();
            let text_width = ink_rect.width() as f64 / gtk::pango::SCALE as f64;
            let text_height = ink_rect.height() as f64 / gtk::pango::SCALE as f64;

            let angle = (i as f64 * PI / 2.0) - PI / 2.0;
            let tx = cx + (radius - 15.0) * angle.cos();
            let ty = cy + (radius - 15.0) * angle.sin();

            cr.move_to(tx - (text_width / 2.0), ty - (text_height / 2.0));
            pangocairo::functions::show_layout(cr, &layout);
        }

        cr.save().expect("Cairo error");
        cr.translate(cx, cy);
        let target_bearing: f64 = *bearing_draw.borrow();
        cr.rotate(target_bearing.to_radians());

        let marker_dist = radius + 35.0;

        cr.translate(0.0, -marker_dist);
        cr.rotate(-target_bearing.to_radians());
        if let Some(pix) = &qibla_icon {
            let is_dark = adw::StyleManager::default().is_dark();

            if is_dark {
                cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
            } else {
                cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
            }

            if is_dark {
                cr.push_group();

                cr.set_source_pixbuf(pix, -16.0, -16.0);
                cr.paint().expect("Cairo error");

                cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);

                cr.set_operator(gtk::cairo::Operator::In);
                cr.paint().expect("Cairo error");

                cr.pop_group_to_source().expect("Cairo error");
                cr.paint().expect("Cairo error");
            } else {
                cr.set_source_pixbuf(pix, -16.0, -16.0);
                cr.paint().expect("Cairo error");
            }
        } else {
            cr.set_source_rgb(0.1, 0.1, 0.1);
            cr.rectangle(-10.0, -10.0, 20.0, 20.0);
            cr.fill().expect("Cairo error");
        }
        cr.restore().expect("Cairo error");

        cr.save().expect("Cairo error");
        cr.translate(cx, cy);
        let rot: f64 = *rotation_draw.borrow();
        cr.rotate(rot.to_radians());

        cr.set_source_rgba(0.0, 0.0, 0.0, 0.2);
        cr.move_to(0.0, -radius + 10.0);
        cr.line_to(15.0, 0.0);
        cr.line_to(-15.0, 0.0);
        cr.close_path();
        cr.fill().expect("Cairo error");

        cr.set_source_rgb(0.8, 0.2, 0.2);
        cr.move_to(0.0, -radius + 15.0);
        cr.line_to(12.0, 0.0);
        cr.line_to(-12.0, 0.0);
        cr.close_path();
        cr.fill().expect("Cairo error");

        cr.set_source_rgb(0.9, 0.9, 0.9);
        cr.move_to(0.0, radius - 15.0);
        cr.line_to(12.0, 0.0);
        cr.line_to(-12.0, 0.0);
        cr.close_path();
        cr.fill().expect("Cairo error");

        cr.restore().expect("Cairo error");

        cr.set_source_rgb(0.3, 0.3, 0.3);
        cr.arc(cx, cy, 5.0, 0.0, 2.0 * PI);
        cr.fill().expect("Cairo error");
    });

    let config_clone = config.clone();
    let compass_clone = compass_manager.clone();
    let drawing_area_clone = drawing_area.clone();
    let current_rot_clone = current_rotation.clone();
    let bearing_clone = bearing.clone();
    let target_rot_clone = Rc::new(RefCell::new(0.0));
    let b_label = bearing_label.clone();
    let s_label = status_label.clone();

    let refresh = Rc::new(move || {
        let cfg = config_clone.borrow();
        let q_bearing = calculate_qibla_bearing(cfg.latitude, cfg.longitude);
        *bearing_clone.borrow_mut() = q_bearing;

        let current_heading = if compass_clone.is_available() {
            compass_clone.get_heading()
        } else {
            0.0
        };

        let target = if compass_clone.is_available() {
            (q_bearing - current_heading + 360.0) % 360.0
        } else {
            q_bearing
        };
        *target_rot_clone.borrow_mut() = target;

        let mut curr = *current_rot_clone.borrow();
        let target_val = *target_rot_clone.borrow();

        let mut diff = target_val - curr;
        if diff > 180.0 {
            diff -= 360.0;
        } else if diff < -180.0 {
            diff += 360.0;
        }

        curr += diff * 0.15;
        curr = (curr + 360.0) % 360.0;
        *current_rot_clone.borrow_mut() = curr;

        let lang = config_clone.borrow().language.clone();
        b_label.set_label(&format!(
            "{:.1}° {}",
            q_bearing,
            tr(get_cardinal(q_bearing), &lang)
        ));
        let status_text = if compass_clone.is_available() {
            "Sensor Active (Smooth)"
        } else {
            "Manual Calculation"
        };
        s_label.set_label(&tr(status_text, &lang));

        drawing_area_clone.queue_draw();
    });

    QiblaPage { container, refresh }
}

fn get_cardinal(bearing: f64) -> &'static str {
    let directions = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    let index = ((bearing + 22.5) / 45.0).floor() as usize % 8;
    directions[index]
}
