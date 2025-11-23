use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Days, Local, LocalResult, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use humantime::parse_duration;

use crate::{
    cli::{AddCommand, DeleteCommand, ListCommand},
    storage::{NewEvent, Storage, StoredEvent},
};

pub fn add_event(storage: &mut Storage, cmd: AddCommand) -> Result<()> {
    let timing = if cmd.all_day {
        if cmd.duration.is_some() {
            return Err(anyhow!("--duration cannot be used with --all-day"));
        }
        parse_all_day_range(&cmd)?
    } else {
        parse_timed_range(&cmd)?
    };

    let new_event = NewEvent {
        title: cmd.title,
        note: cmd.note.unwrap_or_default(),
        starts_at: timing.starts_at,
        ends_at: timing.ends_at,
        all_day: cmd.all_day,
        tags: cmd.tags,
        uid: None,
    };

    let row_id = storage.insert_event(new_event)?;
    println!("Stored event #{row_id}");
    Ok(())
}

pub fn list_events(storage: &Storage, cmd: ListCommand) -> Result<()> {
    let range = if let Some(day) = cmd.day {
        Some(day_range(&day)?)
    } else {
        None
    };
    let events = storage.fetch_events(range)?;
    let tz = parse_timezone(cmd.tz.as_deref())?;

    if events.is_empty() {
        println!("No events found");
        return Ok(());
    }

    for event in events {
        let timing = format_event_timing(&event, &tz)?;
        println!("#{} {}", event.id, event.title);
        println!("  {timing}");
        if !event.tags.is_empty() {
            println!("  tags: {}", event.tags.join(", "));
        }
        if !event.note.is_empty() {
            println!("  note: {}", event.note);
        }
        println!();
    }

    Ok(())
}

pub fn delete_event(storage: &mut Storage, cmd: DeleteCommand) -> Result<()> {
    match (cmd.id, cmd.title.as_deref()) {
        (Some(id), None) => {
            let removed = storage.delete_by_id(id)?;
            if removed {
                println!("Deleted event #{id}");
            } else {
                println!("No event found with id {id}");
            }
        }
        (None, Some(title)) => {
            let removed = storage.delete_by_title(title)?;
            if removed > 0 {
                println!("Deleted {removed} event(s) titled '{title}'");
            } else {
                println!("No events found titled '{title}'");
            }
        }
        (Some(id), Some(title)) => {
            let removed = storage.delete_by_id(id)?;
            if removed {
                println!("Deleted event #{id} titled '{title}'");
            } else {
                println!("No event #{id}; attempting title deletion");
                let removed = storage.delete_by_title(title)?;
                println!("Deleted {removed} event(s) titled '{title}'");
            }
        }
        (None, None) => return Err(anyhow!("Provide either --id or --title")),
    }
    Ok(())
}

pub(super) fn format_event_timing(event: &StoredEvent, zone: &DisplayZone) -> Result<String> {
    let start_utc = parse_utc(&event.starts_at)?;
    let end_utc = parse_utc(&event.ends_at)?;
    match zone {
        DisplayZone::Local => {
            if event.all_day {
                let start = start_utc.with_timezone(&Local);
                let end = end_utc.with_timezone(&Local);
                let end_inclusive = end
                    .date_naive()
                    .pred_opt()
                    .unwrap_or_else(|| end.date_naive());
                Ok(format!(
                    "{} -> {} (all-day, local)",
                    start.date_naive(),
                    end_inclusive
                ))
            } else {
                let start = start_utc.with_timezone(&Local);
                let end = end_utc.with_timezone(&Local);
                Ok(format!(
                    "{} -> {} ({})",
                    start.format("%Y-%m-%d %H:%M %Z"),
                    end.format("%Y-%m-%d %H:%M %Z"),
                    start.offset()
                ))
            }
        }
        DisplayZone::Named(tz) => {
            if event.all_day {
                let start = start_utc.with_timezone(tz);
                let end = end_utc.with_timezone(tz);
                let end_inclusive = end
                    .date_naive()
                    .pred_opt()
                    .unwrap_or_else(|| end.date_naive());
                Ok(format!(
                    "{} -> {} (all-day, {})",
                    start.date_naive(),
                    end_inclusive,
                    tz
                ))
            } else {
                let start = start_utc.with_timezone(tz);
                let end = end_utc.with_timezone(tz);
                Ok(format!(
                    "{} -> {} ({})",
                    start.format("%Y-%m-%d %H:%M %Z"),
                    end.format("%Y-%m-%d %H:%M %Z"),
                    tz
                ))
            }
        }
    }
}

pub(super) fn parse_timezone(input: Option<&str>) -> Result<DisplayZone> {
    if let Some(value) = input {
        let tz = value
            .parse::<Tz>()
            .map_err(|_| anyhow!("unknown timezone '{value}'"))?;
        Ok(DisplayZone::Named(tz))
    } else {
        Ok(DisplayZone::Local)
    }
}

pub(super) fn parse_utc(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("invalid timestamp '{value}'"))?
        .with_timezone(&Utc))
}

fn parse_all_day_range(cmd: &AddCommand) -> Result<EventTiming> {
    let start_value = cmd
        .start
        .as_deref()
        .or(cmd.date.as_deref())
        .ok_or_else(|| anyhow!("provide --start (date) or --date for all-day events"))?;
    let start_date = parse_date(start_value)?;

    let end_date = if let Some(end) = cmd.end.as_deref() {
        parse_date(end)?
    } else {
        start_date
    };

    let start_dt = start_date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("invalid start date"))?
        .and_utc();

    let end_dt = end_date
        .succ_opt()
        .ok_or_else(|| anyhow!("date overflow"))?
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("invalid end date"))?
        .and_utc();

    Ok(EventTiming {
        starts_at: start_dt.to_rfc3339(),
        ends_at: end_dt.to_rfc3339(),
    })
}

fn parse_date(input: &str) -> Result<NaiveDate> {
    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        return Ok(date);
    }
    if let Some(date) = parse_relative_day_with_base(input, Local::now().date_naive()) {
        return Ok(date);
    }
    Err(anyhow!(
        "expected YYYY-MM-DD date (or relative token), got '{input}'"
    ))
}

fn parse_relative_day_with_base(input: &str, base: NaiveDate) -> Option<NaiveDate> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    match trimmed {
        "今日" => return Some(base),
        "明日" => return offset_date(base, 1),
        "昨日" => return offset_date(base, -1),
        _ => {}
    }
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "today" => return Some(base),
        "tomorrow" => return offset_date(base, 1),
        "yesterday" => return offset_date(base, -1),
        _ => {}
    }
    if let Some(days) = parse_english_relative(&lower) {
        return offset_date(base, days);
    }
    if let Some(days) = parse_symbol_relative(&lower) {
        return offset_date(base, days);
    }
    if let Some(days) = parse_japanese_relative(trimmed) {
        return offset_date(base, days);
    }
    None
}

fn parse_english_relative(input: &str) -> Option<i64> {
    if let Some(rest) = input.strip_prefix("in ") {
        let rest = rest.trim();
        if let Some(num_str) = rest.strip_suffix(" days") {
            return num_str.trim().parse().ok().map(|n: i64| n);
        }
        if let Some(num_str) = rest.strip_suffix(" day") {
            return num_str.trim().parse().ok().map(|n: i64| n);
        }
    }
    None
}

fn parse_symbol_relative(input: &str) -> Option<i64> {
    let mut chars = input.chars();
    let sign = chars.next()?;
    if sign != '+' && sign != '-' {
        return None;
    }
    let rest: String = chars.collect();
    let digits = rest.trim_end_matches('d');
    if digits.is_empty() {
        return None;
    }
    let value: i64 = digits.parse().ok()?;
    if sign == '-' {
        Some(-value)
    } else {
        Some(value)
    }
}

fn parse_japanese_relative(input: &str) -> Option<i64> {
    if let Some(value) = input.strip_suffix("日後") {
        return value.trim().parse().ok().map(|n: i64| n);
    }
    if let Some(value) = input.strip_suffix("日前") {
        return value.trim().parse().ok().map(|n: i64| -n);
    }
    None
}

fn offset_date(base: NaiveDate, days: i64) -> Option<NaiveDate> {
    if days >= 0 {
        base.checked_add_days(Days::new(days as u64))
    } else {
        base.checked_sub_days(Days::new((-days) as u64))
    }
}

pub(super) fn day_range(day: &str) -> Result<(String, String)> {
    let date = parse_date(day)?;
    let start = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("invalid day"))?
        .and_utc();
    let end = date
        .succ_opt()
        .ok_or_else(|| anyhow!("date overflow"))?
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("invalid day"))?
        .and_utc();
    Ok((start.to_rfc3339(), end.to_rfc3339()))
}

fn parse_timed_range(cmd: &AddCommand) -> Result<EventTiming> {
    let start_dt = if let Some(start_value) = cmd.start.as_deref() {
        parse_explicit_instant(start_value)?
    } else {
        build_start_from_components(cmd)?
    };

    let end_dt = if let Some(end_value) = cmd.end.as_deref() {
        DateTime::parse_from_rfc3339(end_value)
            .with_context(|| format!("expected RFC3339 timestamp, got '{end_value}'"))?
            .with_timezone(&Utc)
    } else if let Some(duration_value) = cmd.duration.as_deref() {
        let parsed = parse_duration(duration_value)
            .with_context(|| format!("failed to parse duration '{duration_value}'"))?;
        let chrono_dur = chrono::Duration::from_std(parsed)
            .map_err(|_| anyhow!("duration '{duration_value}' is too large"))?;
        start_dt
            .checked_add_signed(chrono_dur)
            .ok_or_else(|| anyhow!("duration pushes end time out of range"))?
    } else {
        start_dt
            .checked_add_signed(chrono::Duration::minutes(30))
            .ok_or_else(|| anyhow!("default duration pushes end time out of range"))?
    };

    if end_dt <= start_dt {
        return Err(anyhow!("--end must be later than --start"));
    }

    Ok(EventTiming {
        starts_at: start_dt.to_rfc3339(),
        ends_at: end_dt.to_rfc3339(),
    })
}

fn parse_explicit_instant(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("expected RFC3339 timestamp, got '{value}'"))?
        .with_timezone(&Utc))
}

fn build_start_from_components(cmd: &AddCommand) -> Result<DateTime<Utc>> {
    let date = resolve_date_or_today(cmd.date.as_deref())?;
    let time_str = cmd
        .time
        .as_deref()
        .ok_or_else(|| anyhow!("provide --time when --start is omitted"))?;
    let time = parse_time_of_day(time_str)?;
    let naive = date.and_time(time);
    let local_dt = match Local.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt,
        LocalResult::Ambiguous(first, second) => {
            return Err(anyhow!(
                "time '{time_str}' is ambiguous ({first} or {second}) due to DST"
            ));
        }
        LocalResult::None => {
            return Err(anyhow!(
                "time '{time_str}' does not exist in the current timezone (DST transition)"
            ));
        }
    };
    Ok(local_dt.with_timezone(&Utc))
}

fn resolve_date_or_today(input: Option<&str>) -> Result<NaiveDate> {
    if let Some(value) = input {
        parse_date(value)
    } else {
        Ok(Local::now().date_naive())
    }
}

fn parse_time_of_day(value: &str) -> Result<NaiveTime> {
    let trimmed = value.trim();
    for fmt in ["%H:%M:%S", "%H:%M"] {
        if let Ok(time) = NaiveTime::parse_from_str(trimmed, fmt) {
            return Ok(time);
        }
    }
    Err(anyhow!(
        "expected HH:MM or HH:MM:SS time-of-day, got '{value}'"
    ))
}

struct EventTiming {
    starts_at: String,
    ends_at: String,
}

pub(super) enum DisplayZone {
    Local,
    Named(Tz),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_keywords_support_multiple_languages() {
        let base = NaiveDate::from_ymd_opt(2025, 5, 1).unwrap();
        assert_eq!(
            parse_relative_day_with_base("tomorrow", base),
            Some(NaiveDate::from_ymd_opt(2025, 5, 2).unwrap())
        );
        assert_eq!(
            parse_relative_day_with_base("+2d", base),
            Some(NaiveDate::from_ymd_opt(2025, 5, 3).unwrap())
        );
        assert_eq!(
            parse_relative_day_with_base("2日後", base),
            Some(NaiveDate::from_ymd_opt(2025, 5, 3).unwrap())
        );
    }
}
