use charming::{
    component::{Axis, Legend},
    element::{AxisLabel, AxisType, Formatter, ItemStyle},
    series::{Bar, Pie, PieRoseType},
    theme::Theme,
    Chart, ImageRenderer,
};
use chrono::{DateTime, Duration, NaiveDate};
use chrono_tz::Tz;
use itertools::Itertools;
use std::{collections::HashMap, mem};

struct DateRange(NaiveDate, NaiveDate);

impl Iterator for DateRange {
    type Item = NaiveDate;
    fn next(&mut self) -> Option<Self::Item> {
        if self.0 <= self.1 {
            let next = self.0 + Duration::days(1);
            Some(mem::replace(&mut self.0, next))
        } else {
            None
        }
    }
}

fn count(strings: Vec<&String>) -> Vec<(f64, &String)> {
    let mut count_map = HashMap::new();
    for string in strings {
        *count_map.entry(string).or_insert(0) += 1;
    }
    count_map
        .into_iter()
        .map(|(s, count)| (count as f64, s))
        .collect()
}

pub fn nightinggale(data: Vec<&String>) -> String {
    let chart = Chart::new().legend(Legend::new().top("bottom")).series(
        Pie::new()
            .name("Tags")
            .rose_type(PieRoseType::Radius)
            .radius(vec!["50", "250"])
            .center(vec!["50%", "50%"])
            .item_style(ItemStyle::new().border_radius(8))
            .data(count(data)),
    );

    let mut renderer = ImageRenderer::new(1000, 800).theme(Theme::Infographic);
    renderer.render(&chart).unwrap()
}

pub fn bar(data: Vec<(i64, i64)>, start_date: DateTime<Tz>, end_date: DateTime<Tz>) -> String {
    let map = data.iter().into_group_map_by(|e| {
        DateTime::from_timestamp(e.0, 0)
            .unwrap()
            .with_timezone(&start_date.timezone())
            .date_naive()
    });

    let zipped = DateRange(start_date.date_naive(), end_date.date_naive()).map(|date| {
        match map
            .get(&date)
            .map(|vec| vec.iter().map(|x| x.1 - x.0).sum::<i64>())
            .map(|sum| (date.to_string(), sum))
        {
            Some(tuple) => tuple,
            None => (date.to_string(), 0),
        }
    });

    let (labels, values) = zipped.into_iter().unzip();

    let chart = Chart::new()
        .x_axis(Axis::new().type_(AxisType::Category).data(labels))
        .y_axis(
            Axis::new().type_(AxisType::Value).axis_label(
                AxisLabel::new().formatter(Formatter::Function(
                    r#"function (param) { 
                        return new Date(param*1000).toLocaleTimeString('en-GB');
                    }"#
                    .into(),
                )),
            ),
        )
        .series(Bar::new().data(values));
    let mut renderer = ImageRenderer::new(1000, 800).theme(Theme::Infographic);
    renderer.render(&chart).unwrap()
}
