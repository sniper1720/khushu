use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::config::AppConfig;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SetupMode {
    #[serde(rename = "by_target_date")]
    ByTargetDate,
    #[serde(rename = "by_daily_amount")]
    ByDailyAmount,
}

impl Default for SetupMode {
    fn default() -> Self {
        SetupMode::ByTargetDate
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ReadingUnit {
    #[serde(rename = "pages")]
    Pages,
    #[serde(rename = "hizb")]
    Hizb,
    #[serde(rename = "juz")]
    Juz,
}

impl Default for ReadingUnit {
    fn default() -> Self {
        ReadingUnit::Pages
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum HifzUnit {
    #[serde(rename = "lines")]
    Lines(u32),
    #[serde(rename = "half_page")]
    HalfPage,
    #[serde(rename = "page")]
    Page,
    #[serde(rename = "custom_pages")]
    CustomPages(u32),
}

impl Default for HifzUnit {
    fn default() -> Self {
        HifzUnit::Page
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum HifzGoalType {
    #[serde(rename = "full_quran")]
    FullQuran,
    #[serde(rename = "selected_juz")]
    SelectedJuz(u32),
    #[serde(rename = "selected_juz_range")]
    SelectedJuzRange { start_juz: u32, end_juz: u32 },
    #[serde(rename = "selected_surah")]
    SelectedSurah(u32),
    #[serde(rename = "selected_surah_range")]
    SelectedSurahRange { start_surah: u32, end_surah: u32 },
    #[serde(rename = "custom_page_range")]
    CustomPageRange { start_page: u32, end_page: u32 },
}

impl Default for HifzGoalType {
    fn default() -> Self {
        HifzGoalType::FullQuran
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DailyStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "partially_completed")]
    PartiallyCompleted { completed_pages: u32 },
    #[serde(rename = "skipped")]
    Skipped,
}

impl Default for DailyStatus {
    fn default() -> Self {
        DailyStatus::Pending
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ReadingDailyRecord {
    pub day_index: u32,
    pub date: String,
    pub start_page: u32,
    pub end_page: u32,
    pub range_label: String,
    pub status: DailyStatus,
    pub completed_at: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ReadingPlanData {
    pub id: String,
    pub title: String,
    pub setup_mode: SetupMode,
    pub unit: ReadingUnit,
    pub start_page: u32,
    pub end_page: u32,
    pub target_days: u32,
    pub daily_amount_target: u32,
    pub start_date: String,
    pub expected_end_date: String,
    pub is_active: bool,
    pub is_archived: bool,
    pub history: Vec<ReadingDailyRecord>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct HifzDailyRecord {
    pub day_index: u32,
    pub date: String,
    pub sabaq_start_page: u32,
    pub sabaq_end_page: u32,
    pub sabqi_start_page: u32,
    pub sabqi_end_page: u32,
    pub manzil_start_page: u32,
    pub manzil_end_page: u32,
    pub sabaq_status: DailyStatus,
    pub sabqi_status: DailyStatus,
    pub manzil_status: DailyStatus,
    pub quality_rating: Option<String>,
    pub mistakes_count: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct HifzPlanData {
    pub id: String,
    pub title: String,
    pub goal_type: HifzGoalType,
    pub setup_mode: SetupMode,
    pub start_page: u32,
    pub end_page: u32,
    pub sabaq_unit: HifzUnit,
    pub sabqi_window_days: u32,
    pub manzil_cycle_days: u32,
    pub target_days: u32,
    pub start_date: String,
    pub expected_end_date: String,
    pub is_active: bool,
    pub is_archived: bool,
    pub history: Vec<HifzDailyRecord>,
}

// ── Persistence Abstraction Layer ──────────────────────────────────────────

pub trait PlanStore {
    fn load_reading_plans(&self) -> Vec<ReadingPlanData>;
    fn save_reading_plans(&self, plans: &[ReadingPlanData]);

    fn load_hifz_plans(&self) -> Vec<HifzPlanData>;
    fn save_hifz_plans(&self, plans: &[HifzPlanData]);
}

pub struct ConfigPlanStore<'a> {
    config: &'a AppConfig,
}

impl<'a> ConfigPlanStore<'a> {
    pub fn new(config: &'a AppConfig) -> Self {
        Self { config }
    }
}

impl<'a> PlanStore for ConfigPlanStore<'a> {
    fn load_reading_plans(&self) -> Vec<ReadingPlanData> {
        self.config.quran_reading_plans()
    }
    fn save_reading_plans(&self, plans: &[ReadingPlanData]) {
        self.config.set_quran_reading_plans(plans.to_vec());
        self.config.save();
    }

    fn load_hifz_plans(&self) -> Vec<HifzPlanData> {
        self.config.quran_hifz_plans()
    }
    fn save_hifz_plans(&self, plans: &[HifzPlanData]) {
        self.config.set_quran_hifz_plans(plans.to_vec());
        self.config.save();
    }
}

// ── Calculations & Logic ───────────────────────────────────────────────────

pub fn calculate_reading_workload(
    start_page: u32,
    end_page: u32,
    setup_mode: &SetupMode,
    target_days: u32,
    daily_amount: u32,
    unit: &ReadingUnit,
    start_date: NaiveDate,
) -> (u32, u32, NaiveDate, Vec<ReadingDailyRecord>) {
    let total_pages = if end_page >= start_page {
        end_page - start_page + 1
    } else {
        1
    };

    let (effective_days, effective_daily_pages) = match setup_mode {
        SetupMode::ByTargetDate => {
            let days = target_days.max(1);
            let pages_per_day = (total_pages as f32 / days as f32).ceil() as u32;
            (days, pages_per_day)
        }
        SetupMode::ByDailyAmount => {
            let multiplier = match unit {
                ReadingUnit::Pages => 1,
                ReadingUnit::Hizb => 10, // ~10 pages per Hizb
                ReadingUnit::Juz => 20,  // ~20 pages per Juz
            };
            let pages_per_day = (daily_amount * multiplier).max(1);
            let days = (total_pages as f32 / pages_per_day as f32).ceil() as u32;
            (days.max(1), pages_per_day)
        }
    };

    let mut history = Vec::new();
    let base_pages = total_pages / effective_days;
    let remainder = total_pages % effective_days;

    let mut current_page = start_page;
    for day in 0..effective_days {
        let pages_today = base_pages + if day < remainder { 1 } else { 0 };
        if pages_today == 0 {
            break;
        }
        let day_start = current_page;
        let day_end = (current_page + pages_today - 1).min(end_page);
        let date_str = (start_date + Duration::days(day as i64))
            .format("%Y-%m-%d")
            .to_string();

        let label = format!("{}–{}", day_start, day_end);
        history.push(ReadingDailyRecord {
            day_index: day + 1,
            date: date_str,
            start_page: day_start,
            end_page: day_end,
            range_label: label,
            status: DailyStatus::Pending,
            completed_at: None,
        });

        current_page = day_end + 1;
        if current_page > end_page {
            break;
        }
    }

    let end_date = start_date + Duration::days((history.len() as i64 - 1).max(0));
    (
        history.len() as u32,
        effective_daily_pages,
        end_date,
        history,
    )
}

pub fn hifz_unit_to_pages(unit: &HifzUnit) -> f32 {
    match unit {
        HifzUnit::Lines(n) => *n as f32 / 15.0, // Standard Mushaf has 15 lines per page
        HifzUnit::HalfPage => 0.5,
        HifzUnit::Page => 1.0,
        HifzUnit::CustomPages(p) => (*p as f32).max(0.1),
    }
}

pub fn get_juz_page_range(juz: u32) -> (u32, u32) {
    let j = juz.clamp(1, 30);
    let start = if j == 1 { 1 } else { (j - 1) * 20 + 2 };
    let end = if j == 30 { 604 } else { j * 20 + 1 };
    (start.min(604), end.min(604))
}

pub fn map_hifz_goal_scope(goal: &HifzGoalType) -> (u32, u32) {
    match goal {
        HifzGoalType::FullQuran => (1, 604),
        HifzGoalType::SelectedJuz(juz) => get_juz_page_range(*juz),
        HifzGoalType::SelectedJuzRange { start_juz, end_juz } => {
            let sj = (*start_juz).clamp(1, 30);
            let ej = (*end_juz).clamp(sj, 30);
            let (s, _) = get_juz_page_range(sj);
            let (_, e) = get_juz_page_range(ej);
            (s, e)
        }
        HifzGoalType::SelectedSurah(surah) => {
            let s = (*surah).clamp(1, 114);
            let start_page = crate::quran::get_surah_start_page(s).unwrap_or(1);
            let end_page = if s < 114 {
                crate::quran::get_surah_start_page(s + 1).unwrap_or(605) - 1
            } else {
                604
            };
            (start_page, end_page.clamp(start_page, 604))
        }
        HifzGoalType::SelectedSurahRange {
            start_surah,
            end_surah,
        } => {
            let s = (*start_surah).clamp(1, 114);
            let e = (*end_surah).clamp(s, 114);
            let start_page = crate::quran::get_surah_start_page(s).unwrap_or(1);
            let end_page = if e < 114 {
                crate::quran::get_surah_start_page(e + 1).unwrap_or(605) - 1
            } else {
                604
            };
            (start_page, end_page.clamp(start_page, 604))
        }
        HifzGoalType::CustomPageRange {
            start_page,
            end_page,
        } => {
            let s = (*start_page).clamp(1, 604);
            let e = (*end_page).clamp(s, 604);
            (s, e)
        }
    }
}

pub fn format_hifz_goal_title(goal: &HifzGoalType, lang: &str) -> String {
    match goal {
        HifzGoalType::FullQuran => crate::i18n::tr("Full Quran"),
        HifzGoalType::SelectedJuz(juz) => {
            format!("{} {}", crate::i18n::tr("Juz"), juz)
        }
        HifzGoalType::SelectedJuzRange { start_juz, end_juz } => {
            format!("{} {}–{}", crate::i18n::tr("Juz Range"), start_juz, end_juz)
        }
        HifzGoalType::SelectedSurah(surah) => {
            let surahs = crate::quran::get_surah_display_list(lang);
            let idx = (surah.saturating_sub(1) as usize).min(surahs.len().saturating_sub(1));
            surahs
                .get(idx)
                .cloned()
                .unwrap_or_else(|| format!("Surah {}", surah))
        }
        HifzGoalType::SelectedSurahRange {
            start_surah,
            end_surah,
        } => {
            let surahs = crate::quran::get_surah_display_list(lang);
            let idx1 = (start_surah.saturating_sub(1) as usize).min(surahs.len().saturating_sub(1));
            let idx2 = (end_surah.saturating_sub(1) as usize).min(surahs.len().saturating_sub(1));
            let s1 = surahs
                .get(idx1)
                .cloned()
                .unwrap_or_else(|| format!("{}", start_surah));
            let s2 = surahs
                .get(idx2)
                .cloned()
                .unwrap_or_else(|| format!("{}", end_surah));
            format!("{} – {}", s1, s2)
        }
        HifzGoalType::CustomPageRange { .. } => crate::i18n::tr("Custom Page Range"),
    }
}

pub fn get_active_daily_record(plan: &ReadingPlanData) -> ReadingDailyRecord {
    let today_str = chrono::Local::now().format("%Y-%m-%d").to_string();

    if let Some(rec) = plan
        .history
        .iter()
        .find(|r| r.date == today_str && r.status != DailyStatus::Completed)
    {
        return rec.clone();
    }

    if let Some(rec) = plan
        .history
        .iter()
        .find(|r| r.status != DailyStatus::Completed)
    {
        return rec.clone();
    }

    plan.history
        .last()
        .cloned()
        .unwrap_or_else(|| ReadingDailyRecord {
            day_index: 1,
            date: today_str,
            start_page: plan.start_page,
            end_page: plan.start_page,
            range_label: format!("Page {}", plan.start_page),
            status: DailyStatus::Completed,
            completed_at: None,
        })
}

pub fn get_active_hifz_record(plan: &HifzPlanData) -> HifzDailyRecord {
    let today_str = chrono::Local::now().format("%Y-%m-%d").to_string();

    if let Some(rec) = plan
        .history
        .iter()
        .find(|r| r.date == today_str && r.sabaq_status != DailyStatus::Completed)
    {
        return rec.clone();
    }

    if let Some(rec) = plan
        .history
        .iter()
        .find(|r| r.sabaq_status != DailyStatus::Completed)
    {
        return rec.clone();
    }

    plan.history
        .last()
        .cloned()
        .unwrap_or_else(|| HifzDailyRecord {
            day_index: 1,
            date: today_str,
            sabaq_start_page: plan.start_page,
            sabaq_end_page: plan.start_page,
            sabqi_start_page: plan.start_page,
            sabqi_end_page: plan.start_page,
            manzil_start_page: plan.start_page,
            manzil_end_page: plan.start_page,
            sabaq_status: DailyStatus::Completed,
            sabqi_status: DailyStatus::Completed,
            manzil_status: DailyStatus::Completed,
            quality_rating: None,
            mistakes_count: None,
        })
}

pub fn calculate_hifz_workload(
    goal: &HifzGoalType,
    setup_mode: &SetupMode,
    target_days: u32,
    sabaq_unit: &HifzUnit,
    sabqi_window: u32,
    manzil_cycle: u32,
    start_date: NaiveDate,
) -> (u32, NaiveDate, Vec<HifzDailyRecord>) {
    let (start_page, end_page) = map_hifz_goal_scope(goal);
    let total_pages = (end_page - start_page + 1) as f32;

    let sabaq_pages_per_day = hifz_unit_to_pages(sabaq_unit);

    let effective_days = match setup_mode {
        SetupMode::ByTargetDate => target_days.max(1),
        SetupMode::ByDailyAmount => (total_pages / sabaq_pages_per_day).ceil() as u32,
    }
    .max(1);

    let mut history = Vec::new();
    let mut current_memorized_pages = 0.0f32;

    for day in 0..effective_days {
        let date_str = (start_date + Duration::days(day as i64))
            .format("%Y-%m-%d")
            .to_string();

        let sabaq_start = (start_page as f32 + current_memorized_pages).floor() as u32;
        let sabaq_end = ((sabaq_start as f32 + sabaq_pages_per_day - 1.0).max(sabaq_start as f32))
            .min(end_page as f32)
            .floor() as u32;

        current_memorized_pages += sabaq_pages_per_day;

        // Sabqi: Review past W days of Sabaq
        let sabqi_pages = (sabaq_pages_per_day * sabqi_window as f32).min(current_memorized_pages);
        let sabqi_end = sabaq_start.saturating_sub(1);
        let sabqi_start = (sabqi_end as f32 - sabqi_pages + 1.0)
            .max(start_page as f32)
            .floor() as u32;

        // Manzil: Cycle remaining older memorized material
        let older_memorized = (current_memorized_pages - sabqi_pages).max(0.0);
        let manzil_daily_pages = (older_memorized / manzil_cycle.max(1) as f32).max(1.0);
        let manzil_start = start_page;
        let manzil_end = (manzil_start as f32 + manzil_daily_pages - 1.0)
            .min(sabqi_start as f32)
            .floor() as u32;

        history.push(HifzDailyRecord {
            day_index: day + 1,
            date: date_str,
            sabaq_start_page: sabaq_start.clamp(start_page, end_page),
            sabaq_end_page: sabaq_end.clamp(start_page, end_page),
            sabqi_start_page: sabqi_start.clamp(start_page, end_page),
            sabqi_end_page: sabqi_end.clamp(start_page, end_page),
            manzil_start_page: manzil_start.clamp(start_page, end_page),
            manzil_end_page: manzil_end.clamp(start_page, end_page),
            sabaq_status: DailyStatus::Pending,
            sabqi_status: DailyStatus::Pending,
            manzil_status: DailyStatus::Pending,
            quality_rating: None,
            mistakes_count: None,
        });

        if (start_page as f32 + current_memorized_pages) >= end_page as f32 {
            break;
        }
    }

    let expected_end_date = start_date + Duration::days((history.len() as i64 - 1).max(0));
    (history.len() as u32, expected_end_date, history)
}

pub fn generate_plan_id(prefix: &str) -> String {
    let now = chrono::Local::now();
    format!("{}_{}", prefix, now.format("%Y%m%d_%H%M%S"))
}

pub fn create_new_reading_plan(
    title: String,
    setup_mode: SetupMode,
    unit: ReadingUnit,
    start_page: u32,
    end_page: u32,
    target_days: u32,
    daily_amount_target: u32,
    start_date: NaiveDate,
    expected_end_date: NaiveDate,
    history: Vec<ReadingDailyRecord>,
) -> ReadingPlanData {
    ReadingPlanData {
        id: generate_plan_id("khatma"),
        title,
        setup_mode,
        unit,
        start_page,
        end_page,
        target_days,
        daily_amount_target,
        start_date: start_date.format("%Y-%m-%d").to_string(),
        expected_end_date: expected_end_date.format("%Y-%m-%d").to_string(),
        is_active: true,
        is_archived: false,
        history,
    }
}

pub fn create_new_hifz_plan(
    title: String,
    goal_type: HifzGoalType,
    setup_mode: SetupMode,
    start_page: u32,
    end_page: u32,
    sabaq_unit: HifzUnit,
    sabqi_window_days: u32,
    manzil_cycle_days: u32,
    target_days: u32,
    start_date: NaiveDate,
    expected_end_date: NaiveDate,
    history: Vec<HifzDailyRecord>,
) -> HifzPlanData {
    HifzPlanData {
        id: generate_plan_id("hifz"),
        title,
        goal_type,
        setup_mode,
        start_page,
        end_page,
        sabaq_unit,
        sabqi_window_days,
        manzil_cycle_days,
        target_days,
        start_date: start_date.format("%Y-%m-%d").to_string(),
        expected_end_date: expected_end_date.format("%Y-%m-%d").to_string(),
        is_active: true,
        is_archived: false,
        history,
    }
}

#[allow(dead_code)]
pub fn reschedule_reading_plan(
    plan: &mut ReadingPlanData,
    current_day_index: u32,
    reschedule_uncompleted: bool,
) {
    if plan.history.is_empty() {
        return;
    }

    let mut remaining_uncompleted_pages = 0;
    let mut future_days_count = 0;

    for record in &mut plan.history {
        if record.day_index < current_day_index {
            if let DailyStatus::PartiallyCompleted { completed_pages } = record.status {
                let pages_assigned = record.end_page - record.start_page + 1;
                if pages_assigned > completed_pages {
                    remaining_uncompleted_pages += pages_assigned - completed_pages;
                }
            } else if record.status == DailyStatus::Skipped {
                remaining_uncompleted_pages += record.end_page - record.start_page + 1;
            }
        } else {
            future_days_count += 1;
        }
    }

    if reschedule_uncompleted && remaining_uncompleted_pages > 0 && future_days_count > 0 {
        let additional_per_day =
            (remaining_uncompleted_pages as f32 / future_days_count as f32).ceil() as u32;
        let mut extra_offset = 0;

        for record in &mut plan.history {
            if record.day_index >= current_day_index {
                let current_start = record.start_page + extra_offset;
                let current_end =
                    (record.end_page + extra_offset + additional_per_day).min(plan.end_page);
                record.start_page = current_start;
                record.end_page = current_end;
                record.range_label = format!("{}–{}", current_start, current_end);
                extra_offset += additional_per_day;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodayWirdInfo {
    pub plan_id: String,
    pub plan_title: String,
    pub day_index: u32,
    pub date_str: String,
    pub start_page: u32,
    pub end_page: u32,
    pub total_pages: u32,
    pub completed_pages: u32,
    pub remaining_pages: u32,
    pub target_page: u32,
    pub is_completed: bool,
}

pub fn get_today_wird_info(config: &AppConfig) -> Option<TodayWirdInfo> {
    let store = ConfigPlanStore::new(config);
    let plans = store.load_reading_plans();
    let active_plan = plans.into_iter().find(|p| p.is_active && !p.is_archived)?;

    let record = get_active_daily_record(&active_plan);
    let total_pages = if record.end_page >= record.start_page {
        record.end_page - record.start_page + 1
    } else {
        1
    };

    let last_page = config.quran_last_page().unwrap_or(0);
    let (completed_pages, target_page) = if record.status == DailyStatus::Completed {
        (total_pages, record.start_page)
    } else if last_page >= record.start_page && last_page <= record.end_page {
        let comp = (last_page - record.start_page + 1).min(total_pages);
        let tgt = (last_page + 1).min(record.end_page);
        (comp, tgt)
    } else if last_page > record.end_page {
        (total_pages, record.start_page)
    } else {
        (0, record.start_page)
    };

    let is_completed = record.status == DailyStatus::Completed || completed_pages >= total_pages;
    let remaining_pages = if is_completed {
        0
    } else {
        total_pages.saturating_sub(completed_pages)
    };

    Some(TodayWirdInfo {
        plan_id: active_plan.id,
        plan_title: active_plan.title,
        day_index: record.day_index,
        date_str: record.date.clone(),
        start_page: record.start_page,
        end_page: record.end_page,
        total_pages,
        completed_pages,
        remaining_pages,
        target_page,
        is_completed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reading_workload_30_days_full_quran() {
        let start = NaiveDate::from_ymd_opt(2026, 8, 16).unwrap();
        let (days, _, end_date, history) = calculate_reading_workload(
            1,
            604,
            &SetupMode::ByTargetDate,
            30,
            20,
            &ReadingUnit::Pages,
            start,
        );

        assert_eq!(days, 30);
        assert_eq!(history.len(), 30);
        assert_eq!(history[0].start_page, 1);
        assert_eq!(history[0].end_page, 21); // 604 % 30 = 4 extra pages, so first 4 days get 21 pages
        assert_eq!(history[29].end_page, 604);
        assert_eq!(end_date, start + Duration::days(29));
    }

    #[test]
    fn test_reading_workload_by_daily_amount_juz() {
        let start = NaiveDate::from_ymd_opt(2026, 8, 16).unwrap();
        let (days, pages_per_day, _, history) = calculate_reading_workload(
            1,
            604,
            &SetupMode::ByDailyAmount,
            30,
            1, // 1 Juz/day = 20 pages/day
            &ReadingUnit::Juz,
            start,
        );

        assert_eq!(pages_per_day, 20);
        assert_eq!(days, 31); // 604 / 20 = 30.2 -> 31 days
        assert_eq!(history[0].start_page, 1);
        assert_eq!(history[0].end_page, 20);
    }

    #[test]
    fn test_hifz_workload_half_page_per_day() {
        let start = NaiveDate::from_ymd_opt(2026, 8, 16).unwrap();
        let (days, _, history) = calculate_hifz_workload(
            &HifzGoalType::SelectedJuz(30), // 20 pages
            &SetupMode::ByDailyAmount,
            30,
            &HifzUnit::HalfPage,
            7,
            7,
            start,
        );

        assert_eq!(days, 44);
        assert_eq!(history.len(), 44);
        assert_eq!(history[0].sabaq_start_page, 582);
    }

    #[test]
    fn test_reading_reschedule_unfinished() {
        let start = NaiveDate::from_ymd_opt(2026, 8, 16).unwrap();
        let (_, _, _, history) = calculate_reading_workload(
            1,
            100,
            &SetupMode::ByTargetDate,
            10,
            10,
            &ReadingUnit::Pages,
            start,
        );

        let mut plan = ReadingPlanData {
            id: "test_plan".to_string(),
            title: "Test Plan".to_string(),
            setup_mode: SetupMode::ByTargetDate,
            unit: ReadingUnit::Pages,
            start_page: 1,
            end_page: 100,
            target_days: 10,
            daily_amount_target: 10,
            start_date: "2026-08-16".to_string(),
            expected_end_date: "2026-08-25".to_string(),
            is_active: true,
            is_archived: false,
            history,
        };

        // Mark day 1 as skipped
        plan.history[0].status = DailyStatus::Skipped;

        reschedule_reading_plan(&mut plan, 2, true);

        // Day 2 should now have an increased end page to catch up
        assert!(plan.history[1].end_page > 20);
    }

    #[test]
    fn test_generate_plan_id_safety() {
        let id_khatma = generate_plan_id("khatma");
        assert!(id_khatma.starts_with("khatma_"));
        assert!(id_khatma.len() >= 20);

        let id_hifz = generate_plan_id("hifz");
        assert!(id_hifz.starts_with("hifz_"));
        assert!(id_hifz.len() >= 18);
    }

    #[test]
    fn test_active_daily_record_advancement_and_idempotency() {
        let start = NaiveDate::from_ymd_opt(2026, 8, 16).unwrap();
        let (_, _, _, history) = calculate_reading_workload(
            1,
            604,
            &SetupMode::ByTargetDate,
            30,
            20,
            &ReadingUnit::Pages,
            start,
        );

        let mut plan = ReadingPlanData {
            id: "test_khatma".to_string(),
            title: "Test Khatma".to_string(),
            setup_mode: SetupMode::ByTargetDate,
            unit: ReadingUnit::Pages,
            start_page: 1,
            end_page: 604,
            target_days: 30,
            daily_amount_target: 20,
            start_date: "2026-08-16".to_string(),
            expected_end_date: "2026-09-14".to_string(),
            is_active: true,
            is_archived: false,
            history,
        };

        // Day 1 active
        let active1 = get_active_daily_record(&plan);
        assert_eq!(active1.day_index, 1);
        assert_eq!(active1.start_page, 1);

        // Complete Day 1
        plan.history[0].status = DailyStatus::Completed;

        // Active should now advance to Day 2
        let active2 = get_active_daily_record(&plan);
        assert_eq!(active2.day_index, 2);

        // Repeating completion on Day 1 is idempotent
        plan.history[0].status = DailyStatus::Completed;
        let active_repeat = get_active_daily_record(&plan);
        assert_eq!(active_repeat.day_index, 2);
    }

    #[test]
    fn test_hifz_goal_scopes_mapping() {
        assert_eq!(map_hifz_goal_scope(&HifzGoalType::FullQuran), (1, 604));
        assert_eq!(map_hifz_goal_scope(&HifzGoalType::SelectedJuz(1)), (1, 21));
        assert_eq!(
            map_hifz_goal_scope(&HifzGoalType::SelectedJuz(15)),
            (282, 301)
        );
        assert_eq!(
            map_hifz_goal_scope(&HifzGoalType::SelectedJuz(30)),
            (582, 604)
        );

        let custom = map_hifz_goal_scope(&HifzGoalType::CustomPageRange {
            start_page: 120,
            end_page: 145,
        });
        assert_eq!(custom, (120, 145));
    }

    #[test]
    fn test_today_wird_info_resolution_and_partial_progress() {
        let config = AppConfig::default();
        config.set_quran_reminder_enabled(true);
        config.set_quran_startup_reminder_enabled(true);
        config.set_quran_later_reminder_enabled(true);
        config.set_quran_later_reminder_time("15:00".to_string());
        assert!(config.quran_reminder_enabled());
        assert_eq!(config.quran_later_reminder_time(), "15:00");

        let store = ConfigPlanStore::new(&config);
        store.save_reading_plans(&[]);
        assert!(get_today_wird_info(&config).is_none());

        let today = chrono::Local::now().date_naive();
        let (days, daily, expected_end, history) = calculate_reading_workload(
            1,
            604,
            &SetupMode::ByTargetDate,
            30,
            20,
            &ReadingUnit::Pages,
            today,
        );
        let plan = create_new_reading_plan(
            "Test Khatma".to_string(),
            SetupMode::ByTargetDate,
            ReadingUnit::Pages,
            1,
            604,
            days,
            daily,
            today,
            expected_end,
            history,
        );
        store.save_reading_plans(&[plan]);

        let info = get_today_wird_info(&config).expect("Should find active Wird");
        assert_eq!(info.day_index, 1);
        assert_eq!(info.start_page, 1);
        assert_eq!(info.total_pages, 21);
        assert_eq!(info.completed_pages, 0);
        assert_eq!(info.remaining_pages, 21);
        assert_eq!(info.target_page, 1);
        assert!(!info.is_completed);

        config.set_quran_last_page(Some(8));
        let partial_info = get_today_wird_info(&config).unwrap();
        assert_eq!(partial_info.completed_pages, 8);
        assert_eq!(partial_info.remaining_pages, 13);
        assert_eq!(partial_info.target_page, 9);
        assert!(!partial_info.is_completed);

        let mut plans = store.load_reading_plans();
        plans[0].history[0].status = DailyStatus::Completed;
        store.save_reading_plans(&plans);

        let completed_info = get_today_wird_info(&config).unwrap();
        assert_eq!(completed_info.day_index, 2);
    }

    #[test]
    fn test_quran_reminder_navigation_and_duplicate_suppression() {
        let config = AppConfig::default();
        config.set_quran_reminder_enabled(true);
        config.set_quran_startup_reminder_enabled(true);
        config.set_quran_later_reminder_enabled(true);
        config.set_quran_later_reminder_time("14:00".to_string());

        let store = ConfigPlanStore::new(&config);
        let today = chrono::Local::now().date_naive();
        let (days, daily, expected_end, history) = calculate_reading_workload(
            215,
            234,
            &SetupMode::ByTargetDate,
            1,
            20,
            &ReadingUnit::Pages,
            today,
        );
        let plan = create_new_reading_plan(
            "Test Khatma Range".to_string(),
            SetupMode::ByTargetDate,
            ReadingUnit::Pages,
            215,
            234,
            days,
            daily,
            today,
            expected_end,
            history,
        );
        store.save_reading_plans(&[plan]);

        // Scenario 5: Start Reading resolves correct start page (215)
        let wird_start = get_today_wird_info(&config).expect("Must find active Wird");
        assert_eq!(wird_start.start_page, 215);
        assert_eq!(wird_start.end_page, 234);
        assert_eq!(wird_start.target_page, 215);
        assert_eq!(wird_start.completed_pages, 0);
        assert_eq!(wird_start.remaining_pages, 20);

        // Scenario 6: Partial progress resolves correct remaining page
        config.set_quran_last_page(Some(215));
        let wird_partial = get_today_wird_info(&config).unwrap();
        assert_eq!(wird_partial.completed_pages, 1);
        assert_eq!(wird_partial.remaining_pages, 19);
        assert_eq!(wird_partial.target_page, 216);

        // Scenario 7 & 8: Navigation request survives and does not reset
        crate::quran::request_quran_page_navigation(216);
        assert_eq!(crate::quran::take_requested_quran_page(), Some(216));
        assert_eq!(crate::quran::take_requested_quran_page(), None);

        // Scenario 3 & 4: Completed Wird suppresses remaining pages
        let mut plans = store.load_reading_plans();
        plans[0].history[0].status = DailyStatus::Completed;
        store.save_reading_plans(&plans);

        let wird_done = get_today_wird_info(&config).unwrap();
        assert!(wird_done.is_completed);
        assert_eq!(wird_done.remaining_pages, 0);
    }

    #[test]
    fn test_hifz_all_6_goal_scopes_and_formatting() {
        let full = HifzGoalType::FullQuran;
        assert_eq!(map_hifz_goal_scope(&full), (1, 604));

        let juz15 = HifzGoalType::SelectedJuz(15);
        assert_eq!(map_hifz_goal_scope(&juz15), (282, 301));

        let juz_range = HifzGoalType::SelectedJuzRange {
            start_juz: 1,
            end_juz: 5,
        };
        assert_eq!(map_hifz_goal_scope(&juz_range), (1, 101));

        let surah_baqarah = HifzGoalType::SelectedSurah(2);
        assert_eq!(map_hifz_goal_scope(&surah_baqarah), (2, 49));

        let surah_range = HifzGoalType::SelectedSurahRange {
            start_surah: 2,
            end_surah: 3,
        };
        assert_eq!(map_hifz_goal_scope(&surah_range), (2, 76));

        let custom = HifzGoalType::CustomPageRange {
            start_page: 120,
            end_page: 180,
        };
        assert_eq!(map_hifz_goal_scope(&custom), (120, 180));

        assert_eq!(format_hifz_goal_title(&full, "en"), "Full Quran");
        assert_eq!(format_hifz_goal_title(&juz15, "en"), "Juz 15");
        assert_eq!(format_hifz_goal_title(&juz_range, "en"), "Juz Range 1–5");
        assert_eq!(format_hifz_goal_title(&custom, "en"), "Custom Page Range");
    }

    #[test]
    fn test_hifz_daily_progress_completion_and_advancement() {
        let config = AppConfig::default();
        let store = ConfigPlanStore::new(&config);
        let today = chrono::Local::now().date_naive();
        let goal = HifzGoalType::SelectedJuz(30);

        let (calc_days, expected_end, history) = calculate_hifz_workload(
            &goal,
            &SetupMode::ByDailyAmount,
            30,
            &HifzUnit::Page,
            7,
            14,
            today,
        );

        let plan = create_new_hifz_plan(
            "Test Hifz Plan".to_string(),
            goal,
            SetupMode::ByDailyAmount,
            582,
            604,
            HifzUnit::Page,
            7,
            14,
            calc_days,
            today,
            expected_end,
            history,
        );
        store.save_hifz_plans(&[plan]);

        let loaded = store.load_hifz_plans();
        assert_eq!(loaded.len(), 1);

        let record = get_active_hifz_record(&loaded[0]);
        assert_eq!(record.sabaq_start_page, 582);

        let mut plan_mut = loaded[0].clone();
        plan_mut.history[0].sabaq_status = DailyStatus::Completed;
        plan_mut.history[0].sabqi_status = DailyStatus::Completed;
        plan_mut.history[0].manzil_status = DailyStatus::Completed;
        store.save_hifz_plans(&[plan_mut]);

        let reloaded = store.load_hifz_plans();
        let next_record = get_active_hifz_record(&reloaded[0]);
        assert_eq!(next_record.day_index, 2);
    }

    #[test]
    fn test_hifz_surah_range_resolution_and_validation() {
        let surah_range_2_10 = HifzGoalType::SelectedSurahRange {
            start_surah: 2,
            end_surah: 10,
        };
        let (start_p, end_p) = map_hifz_goal_scope(&surah_range_2_10);
        assert_eq!(start_p, 2);
        assert_eq!(end_p, 221);
        let count = (end_p + 1).saturating_sub(start_p);
        assert_eq!(count, 220);

        let surah_114 = HifzGoalType::SelectedSurah(114);
        let (s114, e114) = map_hifz_goal_scope(&surah_114);
        assert_eq!(s114, 604);
        assert_eq!(e114, 604);

        let invalid_range = HifzGoalType::SelectedSurahRange {
            start_surah: 10,
            end_surah: 2,
        };
        let (s_inv, e_inv) = map_hifz_goal_scope(&invalid_range);
        assert!(s_inv <= e_inv);
    }
}
