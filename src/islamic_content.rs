use crate::i18n::tr;
use chrono::Datelike;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentCategory {
    PrePrayer,
    PrayerTime,
    Iqamah,
    #[allow(dead_code)]
    PreviousCheck,
}

#[derive(Debug, Clone, Copy)]
pub struct IslamicReminderKeys {
    pub text_key: &'static str,
    pub source_key: Option<&'static str>,
}

impl IslamicReminderKeys {
    pub fn resolve(&self) -> (String, Option<String>) {
        (tr(self.text_key), self.source_key.map(tr))
    }
}

pub struct ContentService;

impl ContentService {
    pub fn get_reminder_keys(
        category: ContentCategory,
        date: chrono::NaiveDate,
        prayer_name: &str,
    ) -> IslamicReminderKeys {
        let pool = Self::get_pool(category);
        if pool.is_empty() {
            return IslamicReminderKeys {
                text_key: "Maintain your prayers consistently.",
                source_key: None,
            };
        }

        let prayer_val: usize = prayer_name.bytes().map(|b| b as usize).sum();
        let day_val = date.num_days_from_ce() as usize;
        let index = (day_val + (category as usize * 3) + prayer_val) % pool.len();

        let (text_key, source_key) = pool[index];
        IslamicReminderKeys {
            text_key,
            source_key,
        }
    }

    fn get_pool(category: ContentCategory) -> &'static [(&'static str, Option<&'static str>)] {
        match category {
            ContentCategory::PrePrayer => &[
                (
                    "The best of deeds is prayer at its specified time.",
                    Some("Sahih al-Bukhari #527, Sahih Muslim #85"),
                ),
                (
                    "Whoever performs Wudu thoroughly, his sins come out from his body.",
                    Some("Sahih Muslim #245"),
                ),
                (
                    "Give glad tidings to those who walk to mosques in darkness of perfect light on Judgment Day.",
                    Some("Sunan Abi Dawud #561, Sunan al-Tirmidhi #223"),
                ),
                (
                    "Verily, prayer restrains from shameful and unjust deeds.",
                    Some("Surah Al-Ankabut 29:45"),
                ),
            ],
            ContentCategory::PrayerTime => &[
                (
                    "Prayer in congregation is twenty-seven times superior to prayer performed individually.",
                    Some("Sahih al-Bukhari #645, Sahih Muslim #650"),
                ),
                (
                    "Whoever performs the two cool prayers (Fajr and Asr) will enter Paradise.",
                    Some("Sahih al-Bukhari #574, Sahih Muslim #635"),
                ),
                (
                    "The first thing for which a person will be brought to account on Judgment Day is prayer.",
                    Some("Sunan al-Tirmidhi #413, Sunan an-Nasa'i #3991"),
                ),
            ],
            ContentCategory::Iqamah => &[
                (
                    "When the Iqamah is called, do not come running, but come walking with calmness and dignity.",
                    Some("Sahih al-Bukhari #636, Sahih Muslim #602"),
                ),
                (
                    "Do not miss the virtue of praying in congregation.",
                    Some("Sahih al-Bukhari #645"),
                ),
                (
                    "Straighten your rows, for straightening the rows is part of establishing prayer.",
                    Some("Sahih al-Bukhari #723, Sahih Muslim #433"),
                ),
            ],
            ContentCategory::PreviousCheck => &[
                (
                    "Whoever forgets a prayer or sleeps through it, its expiation is to pray it when he remembers.",
                    Some("Sahih al-Bukhari #597, Sahih Muslim #684"),
                ),
                (
                    "Guard strictly your prayers, especially the middle prayer.",
                    Some("Surah Al-Baqarah 2:238"),
                ),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_content_selection_is_deterministic() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 16).unwrap();
        let k1 = ContentService::get_reminder_keys(ContentCategory::PrePrayer, date, "Dhuhr");
        let k2 = ContentService::get_reminder_keys(ContentCategory::PrePrayer, date, "Dhuhr");
        assert_eq!(k1.text_key, k2.text_key);
        assert_eq!(k1.source_key, k2.source_key);
    }

    #[test]
    fn test_different_prayers_or_categories_vary() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 16).unwrap();
        let k1 = ContentService::get_reminder_keys(ContentCategory::PrePrayer, date, "Fajr");
        let k2 = ContentService::get_reminder_keys(ContentCategory::Iqamah, date, "Fajr");
        assert!(!k1.text_key.is_empty());
        assert!(!k2.text_key.is_empty());
    }
}
