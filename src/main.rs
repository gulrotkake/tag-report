use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use chrono::{
    DateTime, Datelike, Days, Duration, FixedOffset, Months, NaiveDate, NaiveTime, TimeDelta,
    TimeZone, Utc,
};
use chrono_tz::Tz;
use clap::Parser;
use clap_stdin::FileOrStdin;
use serde::Deserialize;
use std::{
    cmp::{max, min},
    error::Error,
    fs,
};
mod visualizations;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    start: Option<DateTime<FixedOffset>>,
    #[arg(short, long)]
    end: Option<DateTime<FixedOffset>>,
    #[arg(default_value = "Europe/Oslo", short, long)]
    timezone: Tz,

    #[arg(short, long)]
    logo: Option<String>,

    csv: FileOrStdin,
}

#[derive(Debug, Deserialize)]
struct Record {
    start: String,
    end: String,
    tags: String,
    description: String,
}

#[derive(bart_derive::BartDisplay)]
#[template = "src/report.template"]
struct Report<'a> {
    start: &'a NaiveDate,
    end: &'a NaiveDate,
    pie: &'a str,
    bar: &'a str,
    duration: &'a str,
    entries: &'a Vec<(&'a String, String)>,
    logo: String,
}

fn split_interval(
    start: &chrono::DateTime<Tz>,
    end: &chrono::DateTime<Tz>,
) -> Vec<(chrono::DateTime<Tz>, chrono::DateTime<Tz>)> {
    let mut current = *start;
    let mut durations = vec![];
    while current <= *end {
        if current.date_naive() == end.date_naive() {
            durations.push((
                current,
                end.with_time(NaiveTime::MIN)
                    .unwrap()
                    .checked_add_signed(*end - current)
                    .unwrap(),
            ));
            break;
        } else {
            let tomorrow = current
                .checked_add_days(Days::new(1))
                .unwrap()
                .with_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                .unwrap();
            durations.push((current, tomorrow));
            current = tomorrow;
        }
    }
    durations
}

fn group_by_day(
    data: &str,
    start_seconds: i64,
    end_seconds: i64,
    tz: &Tz,
) -> Result<Vec<(i64, i64, Vec<String>, String)>, Box<dyn Error>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(data.as_bytes());
    let mut res: Vec<(i64, i64, Vec<String>, String)> = vec![];
    let range = start_seconds..end_seconds;

    for result in rdr.deserialize() {
        let record: Record = result?;
        let start = chrono::DateTime::parse_from_rfc3339(record.start.as_str())
            .unwrap()
            .with_timezone(tz);

        let end = if record.end.is_empty() {
            chrono::Utc::now().with_timezone(tz)
        } else {
            chrono::DateTime::parse_from_rfc3339(record.end.as_str())
                .unwrap()
                .with_timezone(tz)
        };

        if !range.contains(&start.timestamp()) && !range.contains(&end.timestamp()) {
            continue;
        }

        // Does the entry start and stop on the same day?
        if start.date_naive() == end.date_naive() {
            res.push((
                max(start.timestamp(), start_seconds),
                min(end.timestamp(), end_seconds),
                record.tags.split(' ').map(|s| s.to_string()).collect(),
                record.description,
            ));
        } else {
            // Split the entry across days.
            for (start, end) in split_interval(&start, &end) {
                if range.contains(&start.timestamp()) || range.contains(&end.timestamp()) {
                    res.push((
                        max(start.timestamp(), start_seconds),
                        min(end.timestamp(), end_seconds),
                        record.tags.split(' ').map(|s| s.to_string()).collect(),
                        record.description.to_owned(),
                    ))
                }
            }
        }
    }
    Ok(res)
}

fn fmt_duration(seconds: i64) -> String {
    let duration = Duration::seconds(seconds);
    format!(
        "{:0>2}:{:0>2}:{:0>2}",
        (duration.num_seconds() / 60) / 60,
        (duration.num_seconds() / 60) % 60,
        duration.num_seconds() % 60
    )
}

fn get_start_of_month(tz: &Tz) -> DateTime<FixedOffset> {
    let now = Utc::now().with_timezone(tz);
    tz.with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .unwrap()
        .fixed_offset()
}

fn get_end_of_month(tz: &Tz) -> DateTime<FixedOffset> {
    let now = Utc::now().with_timezone(tz);
    return tz
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .unwrap()
        .checked_add_months(Months::new(1))
        .unwrap()
        .checked_sub_signed(TimeDelta::nanoseconds(1))
        .unwrap()
        .fixed_offset();
}

fn main() {
    let args = Args::parse();
    let start = args.start.unwrap_or(get_start_of_month(&args.timezone));
    let end = args.end.unwrap_or(get_end_of_month(&args.timezone));

    let entries = group_by_day(
        &args.csv.contents().unwrap(),
        start.timestamp(),
        end.timestamp(),
        &args.timezone,
    )
    .unwrap();
    let pie = visualizations::nightinggale(entries.iter().flat_map(|entry| &entry.2).collect());

    let bar = visualizations::bar(
        entries.iter().map(|entry| (entry.0, entry.1)).collect(),
        start.with_timezone(&args.timezone),
        end.with_timezone(&args.timezone),
    );

    println!(
        "{}",
        &Report {
            start: &start.with_timezone(&args.timezone).date_naive(),
            end: &end.with_timezone(&args.timezone).date_naive(),
            duration: &fmt_duration(entries.iter().map(|entry| entry.1 - entry.0).sum::<i64>()),
            pie: &pie.replace(r#"<svg width="1000" height="800""#, "<svg"),
            bar: &bar.replace(r#"<svg width="1000" height="800""#, "<svg"),
            entries: &entries
                .iter()
                .map(|entry| (&entry.3, fmt_duration(entry.1 - entry.0)))
                .collect(),
            logo: match args.logo {
                Some(filename) => {
                    format!(
                        "<img src='data:image/svg+xml;base64,{}'>",
                        BASE64_STANDARD.encode(fs::read_to_string(filename).unwrap())
                    )
                }
                None => "".to_owned(),
            }
        }
    );
}

#[cfg(test)]
mod tests {
    use chrono::Datelike;

    use super::*;
    #[test]
    fn date_by_tz() {
        let tz: Tz = "Europe/Oslo".parse().unwrap();
        let d1 = DateTime::from_timestamp(1717365570, 0)
            .unwrap()
            .with_timezone(&tz);
        assert_eq!(d1.date_naive().day(), 2);

        let d2 = DateTime::from_timestamp(1717365600, 0)
            .unwrap()
            .with_timezone(&tz);

        assert_eq!(d2.date_naive().day(), 3);
    }
}
