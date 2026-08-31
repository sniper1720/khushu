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

fn build_cardinal_data(config: &AppConfig) -> CardinalData {
    let texts = [tr("N"), tr("E"), tr("S"), tr("W")];

    let mut font_desc = gtk::pango::FontDescription::new();
    if let Some(family) = config.arabic_font_family() {
        font_desc.set_family(&family);
    }
    font_desc.set_weight(gtk::pango::Weight::Bold);
    font_desc.set_size(12 * gtk::pango::SCALE);

    CardinalData { font_desc, texts }
}

fn compute_bearing(config: &AppConfig, cache: &RefCell<Option<(f64, f64, f64)>>) -> f64 {
    let cached = cache.borrow();
    match *cached {
        Some((latitude, longitude, bearing))
            if latitude == config.latitude() && longitude == config.longitude() =>
        {
            bearing
        }
        _ => {
            drop(cached);
            let bearing = calculate_qibla_bearing(config.latitude(), config.longitude());
            *cache.borrow_mut() = Some((config.latitude(), config.longitude(), bearing));
            bearing
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
    drawing_area: gtk::DrawingArea,
    anim: Rc<RefCell<Option<gtk::glib::SourceId>>>,
) {
    if anim.borrow().is_some() {
        return;
    }
    let anim_inner = anim.clone();
    let source_id = gtk::glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        let mut current = current.borrow_mut();
        let target = *target.borrow();
        let diff = target - *current;
        let angle_delta = if diff > 180.0 {
            diff - 360.0
        } else if diff < -180.0 {
            diff + 360.0
        } else {
            diff
        };
        if angle_delta.abs() < 0.2 {
            *current = target;
            drawing_area.queue_draw();
            *anim_inner.borrow_mut() = None;
            return gtk::glib::ControlFlow::Break;
        }
        *current = (*current + angle_delta * 0.2 + 360.0) % 360.0;
        drawing_area.queue_draw();
        gtk::glib::ControlFlow::Continue
    });
    *anim.borrow_mut() = Some(source_id);
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
    bearing_label: gtk::Label,
    status_label: gtk::Label,
    notify_ids: RefCell<Vec<gtk::glib::SignalHandlerId>>,
    anim_source_id: Rc<RefCell<Option<gtk::glib::SourceId>>>,
    poll_id: RefCell<Option<gtk::glib::SourceId>>,
}

impl QiblaPage {
    pub fn rebuild_cardinals(&self) {
        *self.cardinals.borrow_mut() = build_cardinal_data(&self.config);
        self.update_labels_for_lang();
    }

    pub fn update_labels_for_lang(&self) {
        let bearing = compute_bearing(&self.config, &self.cached_bearing);
        self.bearing_label.set_label(&bearing_label_text(bearing));
        self.status_label
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
        self.rebuild_cardinals();
        let bearing = compute_bearing(&self.config, &self.cached_bearing);

        let target_value = if self.compass.is_available() {
            let heading = self.compass.get_heading();
            (bearing - heading + 360.0) % 360.0
        } else {
            bearing
        };

        *self.target_rotation.borrow_mut() = target_value;

        self.bearing_label.set_label(&bearing_label_text(bearing));
        self.status_label
            .set_label(&status_text(self.compass.is_available()));
        self.drawing_area.queue_draw();
        start_rotation_animation(
            self.current_rotation.clone(),
            self.target_rotation.clone(),
            self.drawing_area.clone(),
            self.anim_source_id.clone(),
        );

        let cached_bearing_latitude = self.cached_bearing.clone();
        let current_rotation_latitude = self.current_rotation.clone();
        let target_rotation_latitude = self.target_rotation.clone();
        let drawing_area_latitude = self.drawing_area.clone();
        let bearing_label_latitude = self.bearing_label.clone();
        let status_label_latitude = self.status_label.clone();
        let anim_c_latitude = self.anim_source_id.clone();
        let compass_latitude = self.compass.clone();
        let latitude_notify_id =
            crate::connect_notify_blocked(&self.config, Some("latitude"), move |config, _| {
                if let Some(active_anim_id) = anim_c_latitude.borrow_mut().take() {
                    active_anim_id.remove();
                }

                *cached_bearing_latitude.borrow_mut() = None;
                let bearing_now = compute_bearing(config, &cached_bearing_latitude);

                let compass_rotation = if compass_latitude.is_available() {
                    let heading = compass_latitude.get_heading();
                    (bearing_now - heading + 360.0) % 360.0
                } else {
                    bearing_now
                };

                *target_rotation_latitude.borrow_mut() = compass_rotation;
                *current_rotation_latitude.borrow_mut() = compass_rotation;

                bearing_label_latitude.set_label(&bearing_label_text(bearing_now));
                status_label_latitude.set_label(&status_text(compass_latitude.is_available()));
                drawing_area_latitude.queue_draw();
            });
        self.notify_ids.borrow_mut().push(latitude_notify_id);

        let cached_bearing_longitude = self.cached_bearing.clone();
        let current_rotation_longitude = self.current_rotation.clone();
        let target_rotation_longitude = self.target_rotation.clone();
        let drawing_area_longitude = self.drawing_area.clone();
        let bearing_label_longitude = self.bearing_label.clone();
        let status_label_longitude = self.status_label.clone();
        let anim_c_longitude = self.anim_source_id.clone();
        let compass_longitude = self.compass.clone();
        let longitude_notify_id =
            crate::connect_notify_blocked(&self.config, Some("longitude"), move |config, _| {
                if let Some(active_anim_id) = anim_c_longitude.borrow_mut().take() {
                    active_anim_id.remove();
                }

                *cached_bearing_longitude.borrow_mut() = None;
                let bearing_now = compute_bearing(config, &cached_bearing_longitude);

                let compass_rotation = if compass_longitude.is_available() {
                    let heading = compass_longitude.get_heading();
                    (bearing_now - heading + 360.0) % 360.0
                } else {
                    bearing_now
                };

                *target_rotation_longitude.borrow_mut() = compass_rotation;
                *current_rotation_longitude.borrow_mut() = compass_rotation;

                bearing_label_longitude.set_label(&bearing_label_text(bearing_now));
                status_label_longitude.set_label(&status_text(compass_longitude.is_available()));
                drawing_area_longitude.queue_draw();
            });
        self.notify_ids.borrow_mut().push(longitude_notify_id);

        let compass_poll = self.compass.clone();
        let config_for_compass_poll = self.config.clone();
        let cached_bearing_poll = self.cached_bearing.clone();
        let current_rotation_poll = self.current_rotation.clone();
        let target_rotation_poll = self.target_rotation.clone();
        let drawing_area_poll = self.drawing_area.clone();
        let bearing_label_poll = self.bearing_label.clone();
        let status_label_poll = self.status_label.clone();
        let anim_poll = self.anim_source_id.clone();
        let last_heading = Rc::new(RefCell::new(0.0f64));
        let poll_id =
            gtk::glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                let heading = compass_poll.get_heading();
                let prev = *last_heading.borrow();
                if (heading - prev).abs() > 0.5 {
                    *last_heading.borrow_mut() = heading;
                    let bearing_now =
                        compute_bearing(&config_for_compass_poll, &cached_bearing_poll);
                    let compass_rotation = if compass_poll.is_available() {
                        (bearing_now - heading + 360.0) % 360.0
                    } else {
                        bearing_now
                    };
                    *target_rotation_poll.borrow_mut() = compass_rotation;
                    bearing_label_poll.set_label(&bearing_label_text(bearing_now));
                    status_label_poll.set_label(&status_text(compass_poll.is_available()));
                    drawing_area_poll.queue_draw();
                    start_rotation_animation(
                        current_rotation_poll.clone(),
                        target_rotation_poll.clone(),
                        drawing_area_poll.clone(),
                        anim_poll.clone(),
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
    let cardinals = Rc::new(RefCell::new(build_cardinal_data(&config)));
    let cardinals_for_draw = cardinals.clone();

    drawing_area.set_draw_func(move |_, cr, width, height| {
        let center_x = width as f64 / 2.0;
        let center_y = height as f64 / 2.0;
        let radius = center_x.min(center_y) - 60.0;

        cr.set_source_rgba(0.5, 0.5, 0.5, 0.3);
        cr.set_line_width(4.0);
        cr.arc(center_x, center_y, radius, 0.0, 2.0 * PI);
        cr.stroke().expect("Cairo error");

        cr.set_source_rgb(0.8, 0.8, 0.8);

        let cardinals_ref = cardinals_for_draw.borrow();
        let pango_ctx = pangocairo::functions::create_context(cr);
        let layout = gtk::pango::Layout::new(&pango_ctx);
        layout.set_font_description(Some(&cardinals_ref.font_desc));

        for (cardinal_index, text) in cardinals_ref.texts.iter().enumerate() {
            layout.set_text(text);
            let (ink_rect, _) = layout.extents();
            let text_width = ink_rect.width() as f64 / gtk::pango::SCALE as f64;
            let text_height = ink_rect.height() as f64 / gtk::pango::SCALE as f64;
            let angle = (cardinal_index as f64 * PI / 2.0) - PI / 2.0;
            let text_x = center_x + (radius - 15.0) * angle.cos();
            let text_y = center_y + (radius - 15.0) * angle.sin();
            cr.move_to(text_x - (text_width / 2.0), text_y - (text_height / 2.0));
            pangocairo::functions::show_layout(cr, &layout);
        }
        drop(cardinals_ref);

        cr.save().expect("Cairo error");
        cr.translate(center_x, center_y);
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
        cr.translate(center_x, center_y);
        let rotation: f64 = *rotation_draw.borrow();
        cr.rotate(rotation.to_radians());

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
        cr.arc(center_x, center_y, 5.0, 0.0, 2.0 * PI);
        cr.fill().expect("Cairo error");
    });

    let refresh = Rc::new({
        let config_for_compass_refresh = config.clone();
        let cached_bearing_c = cached_bearing.clone();
        let current_rotation_c = current_rotation.clone();
        let target_rotation_c = target_rotation.clone();
        let drawing_area_c = drawing_area.clone();
        let bearing_label_c = bearing_label.clone();
        let status_label_c = status_label.clone();
        let compass = compass_manager.clone();
        let anim = anim_source_id.clone();
        move || {
            *cached_bearing_c.borrow_mut() = None;
            let bearing = compute_bearing(&config_for_compass_refresh, &cached_bearing_c);
            let target_value = if compass.is_available() {
                let heading = compass.get_heading();
                (bearing - heading + 360.0) % 360.0
            } else {
                bearing
            };
            *target_rotation_c.borrow_mut() = target_value;
            if let Some(id) = anim.borrow_mut().take() {
                id.remove();
            }
            bearing_label_c.set_label(&bearing_label_text(bearing));
            status_label_c.set_label(&status_text(compass.is_available()));
            drawing_area_c.queue_draw();
            start_rotation_animation(
                current_rotation_c.clone(),
                target_rotation_c.clone(),
                drawing_area_c.clone(),
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
        bearing_label,
        status_label,
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
