use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Days, Duration, Local, LocalResult, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use humantime::parse_duration;

use crate::{
    cli::{AddCommand, DeleteCommand, ListCommand, MoveCommand},
    storage::{NewEvent, Storage, StoredEvent},
};

pub fn add_event(storage: &mut Storage, cmd: AddCommand) -> Result<()> {
    let timing_args = TimingArgs::from_add(&cmd);
    let timing = if cmd.all_day {
        if cmd.duration.is_some() {
            return Err(anyhow!("--duration cannot be used with --all-day"));
        }
        parse_all_day_range(&timing_args, Duration::days(1))?
    } else {
        parse_timed_range(&timing_args, Duration::minutes(30))?
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

pub fn move_event(storage: &mut Storage, cmd: MoveCommand) -> Result<()> {
    let mut event = resolve_move_target(storage, &cmd)?;
    let timing_args = TimingArgs::from_move(&cmd, &event)?;
    if !timing_args.has_explicit_input() {
        return Err(anyhow!(
            "provide --start/--date/--time/--end/--duration to adjust an event"
        ));
    }

    let existing_start = timing_args
        .existing_start
        .expect("move timing should include existing start");
    let existing_end = timing_args
        .existing_end
        .expect("move timing should include existing end");

    let timing = if event.all_day {
        let mut span = existing_end.signed_duration_since(existing_start);
        if span <= Duration::zero() {
            span = Duration::days(1);
        }
        parse_all_day_range(&timing_args, span)?
    } else {
        let mut duration = existing_end.signed_duration_since(existing_start);
        if duration <= Duration::zero() {
            duration = Duration::minutes(1);
        }
        parse_timed_range(&timing_args, duration)?
    };

    if !storage.update_event_timing(event.id, &timing.starts_at, &timing.ends_at, event.all_day)? {
        return Err(anyhow!("failed to update event #{}", event.id));
    }

    event.starts_at = timing.starts_at;
    event.ends_at = timing.ends_at;
    let summary = format_event_timing(&event, &DisplayZone::Local)?;
    println!("Moved event #{} {}", event.id, summary);
    Ok(())
}

fn resolve_move_target(storage: &Storage, cmd: &MoveCommand) -> Result<StoredEvent> {
    match (cmd.id, cmd.title.as_deref()) {
        (Some(id), _) => storage
            .fetch_event_by_id(id)?
            .ok_or_else(|| anyhow!("No event found with id {id}")),
        (None, Some(title)) => {
            let mut matches = storage.fetch_events_by_title(title)?;
            if matches.is_empty() {
                Err(anyhow!("No events found titled '{title}'"))
            } else if matches.len() > 1 {
                Err(anyhow!(
                    "Multiple events titled '{title}'. Use --id to specify which one to move."
                ))
            } else {
                Ok(matches.remove(0))
            }
        }
        (None, None) => Err(anyhow!("Provide either --id or --title")),
    }
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

struct TimingArgs<'a> {
    start: Option<&'a str>,
    date: Option<&'a str>,
    time: Option<&'a str>,
    end: Option<&'a str>,
    duration: Option<&'a str>,
    default_date: NaiveDate,
    existing_start: Option<DateTime<Utc>>,
    existing_end: Option<DateTime<Utc>>,
}

impl<'a> TimingArgs<'a> {
    fn from_add(cmd: &'a AddCommand) -> Self {
        Self {
            start: cmd.start.as_deref(),
            date: cmd.date.as_deref(),
            time: cmd.time.as_deref(),
            end: cmd.end.as_deref(),
            duration: cmd.duration.as_deref(),
            default_date: Local::now().date_naive(),
            existing_start: None,
            existing_end: None,
        }
    }

    fn from_move(cmd: &'a MoveCommand, event: &StoredEvent) -> Result<Self> {
        let existing_start = parse_utc(&event.starts_at)?;
        let existing_end = parse_utc(&event.ends_at)?;
        Ok(Self {
            start: cmd.start.as_deref(),
            date: cmd.date.as_deref(),
            time: cmd.time.as_deref(),
            end: cmd.end.as_deref(),
            duration: cmd.duration.as_deref(),
            default_date: existing_start.with_timezone(&Local).date_naive(),
            existing_start: Some(existing_start),
            existing_end: Some(existing_end),
        })
    }

    fn has_explicit_input(&self) -> bool {
        self.start.is_some()
            || self.date.is_some()
            || self.time.is_some()
            || self.end.is_some()
            || self.duration.is_some()
    }
}

fn parse_all_day_range(args: &TimingArgs<'_>, default_span: Duration) -> Result<EventTiming> {
    let start_date = if let Some(value) = args.start.or(args.date) {
        parse_date(value)?
    } else if let Some(existing) = args.existing_start {
        existing.date_naive()
    } else {
        return Err(anyhow!(
            "provide --start (date) or --date for all-day events"
        ));
    };

    let start_dt = start_date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("invalid start date"))?
        .and_utc();

    let end_dt = if let Some(end_value) = args.end {
        let end_date = parse_date(end_value)?;
        end_date
            .succ_opt()
            .ok_or_else(|| anyhow!("date overflow"))?
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| anyhow!("invalid end date"))?
            .and_utc()
    } else {
        start_dt
            .checked_add_signed(default_span)
            .ok_or_else(|| anyhow!("date overflow"))?
    };

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

fn parse_timed_range(args: &TimingArgs<'_>, default_duration: Duration) -> Result<EventTiming> {
    let start_dt = if let Some(start_value) = args.start {
        parse_explicit_instant(start_value)?
    } else if args.date.is_some() || args.time.is_some() || args.existing_start.is_some() {
        build_start_from_components(args.date, args.time, args.default_date, args.existing_start)?
    } else {
        return Err(anyhow!(
            "provide --start or --date/--time to define a start instant"
        ));
    };

    let end_dt = if let Some(end_value) = args.end {
        DateTime::parse_from_rfc3339(end_value)
            .with_context(|| format!("expected RFC3339 timestamp, got '{end_value}'"))?
            .with_timezone(&Utc)
    } else if let Some(duration_value) = args.duration {
        let parsed = parse_duration(duration_value)
            .with_context(|| format!("failed to parse duration '{duration_value}'"))?;
        let chrono_dur = chrono::Duration::from_std(parsed)
            .map_err(|_| anyhow!("duration '{duration_value}' is too large"))?;
        start_dt
            .checked_add_signed(chrono_dur)
            .ok_or_else(|| anyhow!("duration pushes end time out of range"))?
    } else {
        start_dt
            .checked_add_signed(default_duration)
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

fn build_start_from_components(
    date: Option<&str>,
    time: Option<&str>,
    default_date: NaiveDate,
    fallback_start: Option<DateTime<Utc>>,
) -> Result<DateTime<Utc>> {
    if date.is_none() && time.is_none() {
        return fallback_start.ok_or_else(|| anyhow!("provide --time when --start is omitted"));
    }
    let date_value = if let Some(value) = date {
        parse_date(value)?
    } else if let Some(existing) = fallback_start {
        existing.with_timezone(&Local).date_naive()
    } else {
        default_date
    };
    let time_value = if let Some(value) = time {
        parse_time_of_day(value)?
    } else if let Some(existing) = fallback_start {
        existing.with_timezone(&Local).time()
    } else {
        return Err(anyhow!("provide --time when --start is omitted"));
    };
    let naive = date_value.and_time(time_value);
    let local_dt = match Local.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt,
        LocalResult::Ambiguous(first, second) => {
            return Err(anyhow!(
                "time '{}' is ambiguous ({first} or {second}) due to DST",
                time.unwrap_or("existing")
            ));
        }
        LocalResult::None => {
            return Err(anyhow!(
                "time '{}' does not exist in the current timezone (DST transition)",
                time.unwrap_or("existing")
            ));
        }
    };
    Ok(local_dt.with_timezone(&Utc))
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
