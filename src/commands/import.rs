use std::{fs, io::BufReader};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use ical::property::Property as ParsedProperty;
use ical::{IcalParser, parser::ical::component::IcalEvent as ParsedIcalEvent};

use crate::{
    cli::ImportCommand,
    storage::{NewEvent, Storage},
};

pub fn import_ics(storage: &mut Storage, cmd: ImportCommand) -> Result<()> {
    let path = cmd
        .path
        .as_ref()
        .ok_or_else(|| anyhow!("Provide --path or set import_source in config"))?;
    let file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let reader = BufReader::new(file);
    let parser = IcalParser::new(reader);

    let mut imported = 0usize;
    let mut skipped = 0usize;

    for calendar in parser {
        let calendar = calendar?;
        for event in calendar.events {
            match convert_ical_event(&event) {
                Ok(Some(new_event)) => {
                    let duplicate = new_event
                        .uid
                        .as_deref()
                        .map(|uid| storage.has_event_with_uid(uid))
                        .transpose()?
                        .unwrap_or(false);
                    if duplicate {
                        skipped += 1;
                        continue;
                    }
                    storage.insert_event(new_event)?;
                    imported += 1;
                }
                Ok(None) => skipped += 1,
                Err(err) => {
                    skipped += 1;
                    eprintln!("Skipping event: {err}");
                }
            }
        }
    }

    println!("Imported {imported} event(s), skipped {skipped}");
    Ok(())
}

fn convert_ical_event(event: &ParsedIcalEvent) -> Result<Option<NewEvent>> {
    let (starts_at, all_day) = match get_property(event, "DTSTART") {
        Some(prop) => parse_ics_datetime(prop)?,
        None => return Ok(None),
    };
    let ends_at = parse_ics_end(event, all_day, &starts_at)?;
    let title = get_property(event, "SUMMARY")
        .and_then(parse_text)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Imported event".to_string());
    let note = get_property(event, "DESCRIPTION")
        .and_then(parse_text)
        .unwrap_or_default();
    let tags = get_property(event, "CATEGORIES")
        .and_then(parse_text)
        .map(|text| {
            text.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let uid = get_property(event, "UID").and_then(parse_text);

    Ok(Some(NewEvent {
        title,
        note,
        starts_at: starts_at.to_rfc3339(),
        ends_at: ends_at.to_rfc3339(),
        all_day,
        tags,
        uid,
    }))
}

fn get_property<'a>(event: &'a ParsedIcalEvent, name: &str) -> Option<&'a ParsedProperty> {
    event
        .properties
        .iter()
        .find(|prop| prop.name.eq_ignore_ascii_case(name))
}

fn parse_text(prop: &ParsedProperty) -> Option<String> {
    prop.value.as_ref().map(|value| unescape_ics_text(value))
}

fn parse_ics_datetime(prop: &ParsedProperty) -> Result<(DateTime<Utc>, bool)> {
    let value = prop
        .value
        .as_ref()
        .ok_or_else(|| anyhow!("DTSTART missing value"))?;
    if is_all_day(prop) {
        let date = parse_date_value(value)?;
        let start = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| anyhow!("invalid start date"))?
            .and_utc();
        return Ok((start, true));
    }
    let tzid = property_param(prop, "TZID").map(|s| s.as_str());
    let dt = parse_datetime_value(value, tzid)?;
    Ok((dt, false))
}

fn parse_ics_end(
    event: &ParsedIcalEvent,
    all_day: bool,
    start: &DateTime<Utc>,
) -> Result<DateTime<Utc>> {
    if let Some(prop) = get_property(event, "DTEND") {
        if all_day || is_all_day(prop) {
            let value = prop
                .value
                .as_ref()
                .ok_or_else(|| anyhow!("DTEND missing value"))?;
            let date = parse_date_value(value)?;
            let end = date
                .and_hms_opt(0, 0, 0)
                .ok_or_else(|| anyhow!("invalid end date"))?
                .and_utc();
            return Ok(end);
        } else {
            let tzid = property_param(prop, "TZID").map(|s| s.as_str());
            let dt = parse_datetime_value(
                prop.value
                    .as_deref()
                    .ok_or_else(|| anyhow!("DTEND missing value"))?,
                tzid,
            )?;
            return Ok(dt);
        }
    }

    if all_day {
        Ok(*start + Duration::days(1))
    } else {
        Ok(*start + Duration::hours(1))
    }
}

fn parse_datetime_value(value: &str, tzid: Option<&str>) -> Result<DateTime<Utc>> {
    if let Some(stripped) = value.strip_suffix('Z') {
        let naive = NaiveDateTime::parse_from_str(stripped, "%Y%m%dT%H%M%S")?;
        return Ok(Utc.from_utc_datetime(&naive));
    }
    if let Some(zone) = tzid {
        let tz: Tz = zone
            .parse()
            .map_err(|_| anyhow!("unknown timezone '{zone}'"))?;
        let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S")?;
        let localized = tz
            .from_local_datetime(&naive)
            .single()
            .ok_or_else(|| anyhow!("ambiguous local time for {value} in {zone}"))?;
        return Ok(localized.with_timezone(&Utc));
    }
    let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S")?;
    Ok(Utc.from_utc_datetime(&naive))
}

fn parse_date_value(value: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y%m%d").with_context(|| format!("invalid date '{value}'"))
}

fn property_param<'a>(prop: &'a ParsedProperty, key: &str) -> Option<&'a String> {
    prop.params
        .as_ref()
        .and_then(|params| {
            params
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(key))
                .map(|(_, values)| values)
        })
        .and_then(|vals| vals.first())
}

fn is_all_day(prop: &ParsedProperty) -> bool {
    property_param(prop, "VALUE")
        .map(|v| v.eq_ignore_ascii_case("DATE"))
        .unwrap_or_else(|| prop.value.as_ref().map(|v| v.len() == 8).unwrap_or(false))
}

fn unescape_ics_text(value: &str) -> String {
    value
        .replace("\\\\", "\\")
        .replace("\\n", "\n")
        .replace("\\N", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
}
