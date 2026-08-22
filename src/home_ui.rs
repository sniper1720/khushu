use gtk::Label;
use gtk4 as gtk;
use gtk4::prelude::WidgetExt;

use crate::config::AppConfig;
use crate::location;

fn contains_arabic(text: &str) -> bool {
    text.chars().any(|character| {
        let code = character as u32;
        (0x0600..=0x06FF).contains(&code)
            || (0x0750..=0x077F).contains(&code)
            || (0x08A0..=0x08FF).contains(&code)
            || (0xFB50..=0xFDFF).contains(&code)
            || (0xFE70..=0xFEFF).contains(&code)
    })
}

pub fn refresh_home_ui(
    hijri_label: &Label,
    location_label: &Label,
    language: &str,
    config: &AppConfig,
) {
    let now = crate::time::effective_now(config);
    let hijri_text = crate::time::format_hijri_date(now, config.hijri_offset());
    hijri_label.set_label(&hijri_text);

    let mawaqit_cache = if config.prayer_times_source() == crate::config::PrayerTimesSource::Mawaqit
    {
        config.mawaqit_cache()
    } else {
        None
    };

    if let Some(text) =
        location::display_city_label(config.city_name().as_deref(), mawaqit_cache.as_ref(), language)
    {
        location_label.set_label(&text);
        if contains_arabic(&text) {
            location_label.add_css_class("arabic-text");
        } else {
            location_label.remove_css_class("arabic-text");
        }
    } else {
        location_label.set_label(&format!(
            "{:.2}, {:.2}",
            config.latitude(),
            config.longitude()
        ));
        location_label.remove_css_class("arabic-text");
    }
}
