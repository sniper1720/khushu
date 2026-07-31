use crate::config::AppConfig;
use crate::i18n::tr;
use crate::qibla::{CompassManager, calculate_qibla_bearing};
use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;

struct CardinalData {
    font_desc: gtk::pango::FontDescription,
    texts: [String; 4],
}

fn build_cardinal_data() -> CardinalData {
    let texts = [tr("N"), tr("E"), tr("S"), tr("W")];

    let mut font_desc = gtk::pango::FontDescription::new();
    font_desc.set_family("Amiri, Amiri-Regular");
    font_desc.set_weight(gtk::pango::Weight::Bold);
    font_desc.set_size(12 * gtk::pango::SCALE);

    CardinalData { font_desc, texts }
}

fn compute_bearing(config: &AppConfig, cache: &RefCell<Option<(f64, f64, f64)>>) -> f64 {
    let cached = cache.borrow();
    match *cached {
        Some((lat, lon, b)) if lat == config.latitude() && lon == config.longitude() => b,
        _ => {
            drop(cached);
            let b = calculate_qibla_bearing(config.latitude(), config.longitude());
            *cache.borrow_mut() = Some((config.latitude(), config.longitude(), b));
            b
        }
    }
}

fn bearing_label_text(bearing: f64) -> String {
    format!("{:.1}° {}", bearing, tr(get_cardinal(bearing)))
}

fn status_text(compass_available: bool) -> String {
    if compass_available {
        tr("Sensor Active (Smooth)")
    } else {
        tr("Manual Calculation")
    }
}

fn start_rotation_animation(
    current: Rc<RefCell<f64>>,
    target: Rc<RefCell<f64>>,
    da: gtk::DrawingArea,
    anim: Rc<RefCell<Option<gtk::glib::SourceId>>>,
) {
    if anim.borrow().is_some() {
        return;
    }
    let anim_inner = anim.clone();
    let id = gtk::glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        let mut current = current.borrow_mut();
        let target = *target.borrow();
        let diff = target - *current;
        let dw = if diff > 180.0 {
            diff - 360.0
        } else if diff < -180.0 {
            diff + 360.0
        } else {
            diff
        };
        if dw.abs() < 0.2 {
            *current = target;
            da.queue_draw();
            *anim_inner.borrow_mut() = None;
            return gtk::glib::ControlFlow::Break;
        }
        *current = (*current + dw * 0.2 + 360.0) % 360.0;
        da.queue_draw();
        gtk::glib::ControlFlow::Continue
    });
    *anim.borrow_mut() = Some(id);
}

pub struct QiblaPage {
    pub container: gtk::Box,
    pub refresh: Rc<dyn Fn()>,
    cardinals: Rc<RefCell<CardinalData>>,
    config: AppConfig,
    drawing_area: gtk::DrawingArea,
    compass: Rc<CompassManager>,
    current_rotation: Rc<RefCell<f64>>,
    target_rotation: Rc<RefCell<f64>>,
    cached_bearing: Rc<RefCell<Option<(f64, f64, f64)>>>,
    b_label: gtk::Label,
    s_label: gtk::Label,
    notify_ids: RefCell<Vec<gtk::glib::SignalHandlerId>>,
    anim_source_id: Rc<RefCell<Option<gtk::glib::SourceId>>>,
    poll_id: RefCell<Option<gtk::glib::SourceId>>,
}

impl QiblaPage {
    pub fn rebuild_cardinals(&self) {
        *self.cardinals.borrow_mut() = build_cardinal_data();
        self.update_labels_for_lang();
    }

    pub fn update_labels_for_lang(&self) {
        let q_bearing = compute_bearing(&self.config, &self.cached_bearing);
        self.b_label.set_label(&bearing_label_text(q_bearing));
        self.s_label
            .set_label(&status_text(self.compass.is_available()));
        self.drawing_area.queue_draw();
    }

    pub fn start_listening(&self) {
        for id in self.notify_ids.borrow_mut().drain(..) {
            self.config.disconnect(id);
        }

        if let Some(id) = self.anim_source_id.borrow_mut().take() {
            id.remove();
        }

        *self.cached_bearing.borrow_mut() = None;
        let qb = compute_bearing(&self.config, &self.cached_bearing);

        let tv = if self.compass.is_available() {
            let h = self.compass.get_heading();
            (qb - h + 360.0) % 360.0
        } else {
            qb
        };

        *self.target_rotation.borrow_mut() = tv;

        self.b_label.set_label(&bearing_label_text(qb));
        self.s_label
            .set_label(&status_text(self.compass.is_available()));
        self.drawing_area.queue_draw();
        start_rotation_animation(
            self.current_rotation.clone(),
            self.target_rotation.clone(),
            self.drawing_area.clone(),
            self.anim_source_id.clone(),
        );

        let cached_bearing = self.cached_bearing.clone();
        let current_rotation = self.current_rotation.clone();
        let target_rotation = self.target_rotation.clone();
        let drawing_area = self.drawing_area.clone();
        let bearing_label = self.b_label.clone();
        let status_label = self.s_label.clone();
        let anim_c = self.anim_source_id.clone();
        let compass = self.compass.clone();
        let id = crate::connect_notify_blocked(&self.config, Some("latitude"), move |cfg, _| {
            if let Some(id) = anim_c.borrow_mut().take() {
                id.remove();
            }

            *cached_bearing.borrow_mut() = None;
            let qb = compute_bearing(cfg, &cached_bearing);

            let tv = if compass.is_available() {
                let h = compass.get_heading();
                (qb - h + 360.0) % 360.0
            } else {
                qb
            };

            *target_rotation.borrow_mut() = tv;
            *current_rotation.borrow_mut() = tv;

            bearing_label.set_label(&bearing_label_text(qb));
            status_label.set_label(&status_text(compass.is_available()));
            drawing_area.queue_draw();
        });
        self.notify_ids.borrow_mut().push(id);

        let cached_bearing = self.cached_bearing.clone();
        let current_rotation = self.current_rotation.clone();
        let target_rotation = self.target_rotation.clone();
        let drawing_area = self.drawing_area.clone();
        let bearing_label = self.b_label.clone();
        let status_label = self.s_label.clone();
        let anim_c = self.anim_source_id.clone();
        let compass = self.compass.clone();
        let id = crate::connect_notify_blocked(&self.config, Some("longitude"), move |cfg, _| {
            if let Some(id) = anim_c.borrow_mut().take() {
                id.remove();
            }

            *cached_bearing.borrow_mut() = None;
            let qb = compute_bearing(cfg, &cached_bearing);

            let tv = if compass.is_available() {
                let h = compass.get_heading();
                (qb - h + 360.0) % 360.0
            } else {
                qb
            };

            *target_rotation.borrow_mut() = tv;
            *current_rotation.borrow_mut() = tv;

            bearing_label.set_label(&bearing_label_text(qb));
            status_label.set_label(&status_text(compass.is_available()));
            drawing_area.queue_draw();
        });
        self.notify_ids.borrow_mut().push(id);

        let compass = self.compass.clone();
        let config = self.config.clone();
        let cached_bearing = self.cached_bearing.clone();
        let current_rotation = self.current_rotation.clone();
        let target_rotation = self.target_rotation.clone();
        let drawing_area = self.drawing_area.clone();
        let bearing_label = self.b_label.clone();
        let status_label = self.s_label.clone();
        let anim = self.anim_source_id.clone();
        let last_heading = Rc::new(RefCell::new(0.0f64));
        let poll_id =
            gtk::glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                let heading = compass.get_heading();
                let prev = *last_heading.borrow();
                if (heading - prev).abs() > 0.5 {
                    *last_heading.borrow_mut() = heading;
                    let qb = compute_bearing(&config, &cached_bearing);
                    let tv = if compass.is_available() {
                        (qb - heading + 360.0) % 360.0
                    } else {
                        qb
                    };
                    *target_rotation.borrow_mut() = tv;
                    bearing_label.set_label(&bearing_label_text(qb));
                    status_label.set_label(&status_text(compass.is_available()));
                    drawing_area.queue_draw();
                    start_rotation_animation(
                        current_rotation.clone(),
                        target_rotation.clone(),
                        drawing_area.clone(),
                        anim.clone(),
                    );
                }
                gtk::glib::ControlFlow::Continue
            });
        *self.poll_id.borrow_mut() = Some(poll_id);
    }

    pub fn stop_listening(&self) {
        for id in self.notify_ids.borrow_mut().drain(..) {
            self.config.disconnect(id);
        }
        if let Some(id) = self.anim_source_id.borrow_mut().take() {
            id.remove();
        }
        if let Some(id) = self.poll_id.borrow_mut().take() {
            id.remove();
        }
    }
}

impl Drop for QiblaPage {
    fn drop(&mut self) {
        self.stop_listening();
    }
}

pub fn create_qibla_page(config: AppConfig, compass_manager: Rc<CompassManager>) -> QiblaPage {
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

    let initial_bearing = calculate_qibla_bearing(config.latitude(), config.longitude());

    let bearing_label = gtk::Label::builder()
        .label(bearing_label_text(initial_bearing))
        .css_classes(["title-1"])
        .build();

    let status_label = gtk::Label::builder()
        .label(status_text(false))
        .css_classes(["dim-label"])
        .build();

    container.append(&drawing_area);
    container.append(&bearing_label);
    container.append(&status_label);

    let cached_bearing = Rc::new(RefCell::new(Some((
        config.latitude(),
        config.longitude(),
        initial_bearing,
    ))));
    let current_rotation = Rc::new(RefCell::new(0.0));
    let target_rotation = Rc::new(RefCell::new(0.0));

    let rotation_draw = current_rotation.clone();
    let bearing_draw = target_rotation.clone();

    let qibla_icon = gtk::gdk_pixbuf::Pixbuf::from_resource_at_scale(
        "/io/github/sniper1720/khushu/icons/hicolor/scalable/actions/qibla-symbolic.svg",
        32,
        32,
        true,
    )
    .ok();

    let anim_source_id: Rc<RefCell<Option<gtk::glib::SourceId>>> = Rc::new(RefCell::new(None));
    let cardinals = Rc::new(RefCell::new(build_cardinal_data()));
    let cardinals_for_draw = cardinals.clone();

    drawing_area.set_draw_func(move |_, cr, width, height| {
        let cx = width as f64 / 2.0;
        let cy = height as f64 / 2.0;
        let radius = cx.min(cy) - 60.0;

        cr.set_source_rgba(0.5, 0.5, 0.5, 0.3);
        cr.set_line_width(4.0);
        cr.arc(cx, cy, radius, 0.0, 2.0 * PI);
        cr.stroke().expect("Cairo error");

        cr.set_source_rgb(0.8, 0.8, 0.8);

        let data = cardinals_for_draw.borrow();
        let pango_ctx = pangocairo::functions::create_context(cr);
        let layout = gtk::pango::Layout::new(&pango_ctx);
        layout.set_font_description(Some(&data.font_desc));

        for (i, text) in data.texts.iter().enumerate() {
            layout.set_text(text);
            let (ink_rect, _) = layout.extents();
            let text_width = ink_rect.width() as f64 / gtk::pango::SCALE as f64;
            let text_height = ink_rect.height() as f64 / gtk::pango::SCALE as f64;
            let angle = (i as f64 * PI / 2.0) - PI / 2.0;
            let tx = cx + (radius - 15.0) * angle.cos();
            let ty = cy + (radius - 15.0) * angle.sin();
            cr.move_to(tx - (text_width / 2.0), ty - (text_height / 2.0));
            pangocairo::functions::show_layout(cr, &layout);
        }
        drop(data);

        cr.save().expect("Cairo error");
        cr.translate(cx, cy);
        let bearing_val: f64 = *bearing_draw.borrow();
        cr.rotate(bearing_val.to_radians());

        let marker_dist = radius + 35.0;
        cr.translate(0.0, -marker_dist);
        cr.rotate(-bearing_val.to_radians());

        if let Some(pix) = &qibla_icon {
            let is_dark = adw::StyleManager::default().is_dark();
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

    let refresh = Rc::new({
        let config = config.clone();
        let cached_bearing = cached_bearing.clone();
        let current_rotation = current_rotation.clone();
        let target_rotation = target_rotation.clone();
        let drawing_area = drawing_area.clone();
        let bearing_label = bearing_label.clone();
        let status_label = status_label.clone();
        let compass = compass_manager.clone();
        let anim = anim_source_id.clone();
        move || {
            *cached_bearing.borrow_mut() = None;
            let b = compute_bearing(&config, &cached_bearing);
            let tv = if compass.is_available() {
                let h = compass.get_heading();
                (b - h + 360.0) % 360.0
            } else {
                b
            };
            *target_rotation.borrow_mut() = tv;
            if let Some(id) = anim.borrow_mut().take() {
                id.remove();
            }
            bearing_label.set_label(&bearing_label_text(b));
            status_label.set_label(&status_text(compass.is_available()));
            drawing_area.queue_draw();
            start_rotation_animation(
                current_rotation.clone(),
                target_rotation.clone(),
                drawing_area.clone(),
                anim.clone(),
            );
        }
    });

    QiblaPage {
        container,
        refresh,
        cardinals,
        config,
        drawing_area,
        compass: compass_manager,
        current_rotation,
        target_rotation,
        cached_bearing,
        b_label: bearing_label,
        s_label: status_label,
        notify_ids: RefCell::new(Vec::new()),
        anim_source_id,
        poll_id: RefCell::new(None),
    }
}

fn get_cardinal(bearing: f64) -> &'static str {
    let directions = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    // DIRECTIONS: Cardinal directions — expose to xgettext
    if false {
        tr("N");
        tr("NE");
        tr("E");
        tr("SE");
        tr("S");
        tr("SW");
        tr("W");
        tr("NW");
    }
    let index = ((bearing + 22.5) / 45.0).floor() as usize % 8;
    directions[index]
}
