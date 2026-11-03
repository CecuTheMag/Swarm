//! A network's working hours (spec 0010): when its devices may compute.

use jiff::{Timestamp, ToSpan, Zoned, civil::Time, tz::TimeZone};
use protocol::api::{Hours, HoursWindow};

/// Parses "HH:MM".
fn parse_time(s: &str) -> Option<Time> {
    let (h, m) = s.split_once(':')?;
    let (h, m): (i8, i8) = (h.parse().ok()?, m.parse().ok()?);
    Time::new(h, m, 0, 0).ok()
}

/// Checks the hours a client sent: a known time zone, valid days and times, and not so
/// many windows that evaluating them gets expensive.
pub fn validate(hours: &Hours) -> Result<(), String> {
    TimeZone::get(&hours.timezone)
        .map_err(|_| format!("unknown time zone {:?}", hours.timezone))?;
    if hours.windows.len() > 14 {
        return Err("at most 14 windows".into());
    }
    for w in &hours.windows {
        if w.days.is_empty() || w.days.iter().any(|d| !(1..=7).contains(d)) {
            return Err("days are 1 (Monday) to 7 (Sunday)".into());
        }
        if parse_time(&w.from).is_none() || parse_time(&w.to).is_none() {
            return Err("times are HH:MM".into());
        }
    }
    Ok(())
}

/// Whether `window`, started on the day of `start`, covers the moment `now`.
fn covers(window: &HoursWindow, start: &Zoned, now: &Zoned) -> bool {
    let (Some(from), Some(to)) = (parse_time(&window.from), parse_time(&window.to)) else {
        return false;
    };
    let weekday = start.weekday().to_monday_one_offset() as u8;
    if !window.days.contains(&weekday) {
        return false;
    }
    let Ok(begin) = start
        .date()
        .to_datetime(from)
        .to_zoned(start.time_zone().clone())
    else {
        return false;
    };
    // A window whose end is not after its start runs past midnight.
    let end_day = if to > from {
        start.date()
    } else {
        start.date().tomorrow().unwrap_or(start.date())
    };
    let Ok(end) = end_day.to_datetime(to).to_zoned(start.time_zone().clone()) else {
        return false;
    };
    begin <= *now && *now < end
}

/// Whether devices may compute at `at`. No hours means always.
pub fn open_at(hours: Option<&Hours>, at: Timestamp) -> bool {
    let Some(hours) = hours else { return true };
    let Ok(tz) = TimeZone::get(&hours.timezone) else {
        return true;
    };
    let now = at.to_zoned(tz);
    // A window that started yesterday may still be open (15:00–08:00).
    let yesterday = now.checked_sub(1.day()).unwrap_or_else(|_| now.clone());
    hours
        .windows
        .iter()
        .any(|w| covers(w, &now, &now) || covers(w, &yesterday, &now))
}

/// When the next window opens after `at`, if one does within a week. Checked every
/// quarter hour, which is as fine as the console lets windows be set.
pub fn next_open(hours: &Hours, at: Timestamp) -> Option<Timestamp> {
    let step = 15 * 60;
    let start = at.as_second() - at.as_second().rem_euclid(step) + step;
    (0..7 * 24 * 4)
        .map(|i| start + i * step)
        .filter_map(|s| Timestamp::from_second(s).ok())
        .find(|t| open_at(Some(hours), *t))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sofia(windows: &[(&[u8], &str, &str)]) -> Hours {
        Hours {
            timezone: "Europe/Sofia".into(),
            windows: windows
                .iter()
                .map(|(days, from, to)| HoursWindow {
                    days: days.to_vec(),
                    from: (*from).into(),
                    to: (*to).into(),
                })
                .collect(),
        }
    }

    /// A moment in Sofia's local time.
    fn at(s: &str) -> Timestamp {
        format!("{s}[Europe/Sofia]")
            .parse::<Zoned>()
            .unwrap()
            .timestamp()
    }

    #[test]
    fn no_hours_means_always() {
        assert!(open_at(None, at("2026-11-04T03:00")));
    }

    #[test]
    fn a_daytime_window_on_weekdays() {
        let h = sofia(&[(&[1, 2, 3, 4, 5], "08:00", "15:00")]);
        // 2026-11-04 is a Wednesday, 2026-11-07 a Saturday.
        assert!(open_at(Some(&h), at("2026-11-04T08:00")));
        assert!(open_at(Some(&h), at("2026-11-04T14:59")));
        assert!(!open_at(Some(&h), at("2026-11-04T15:00")));
        assert!(!open_at(Some(&h), at("2026-11-07T10:00")));
    }

    #[test]
    fn a_window_past_midnight_belongs_to_the_day_it_starts() {
        // Lab PCs after school: weekday evenings into the next morning.
        let h = sofia(&[(&[1, 2, 3, 4, 5], "15:00", "08:00")]);
        assert!(open_at(Some(&h), at("2026-11-04T22:00")));
        // Thursday 07:00 is Wednesday's window.
        assert!(open_at(Some(&h), at("2026-11-05T07:00")));
        assert!(!open_at(Some(&h), at("2026-11-05T09:00")));
        // Friday's window reaches into Saturday morning; Saturday's does not exist.
        assert!(open_at(Some(&h), at("2026-11-07T07:59")));
        assert!(!open_at(Some(&h), at("2026-11-07T22:00")));
    }

    #[test]
    fn the_next_window_is_found() {
        let h = sofia(&[(&[1, 2, 3, 4, 5], "15:00", "08:00")]);
        assert_eq!(
            next_open(&h, at("2026-11-04T09:10")),
            Some(at("2026-11-04T15:00"))
        );
        // Saturday morning after 08:00: Monday 15:00.
        assert_eq!(
            next_open(&h, at("2026-11-07T10:00")),
            Some(at("2026-11-09T15:00"))
        );
        assert_eq!(next_open(&sofia(&[]), at("2026-11-04T09:10")), None);
    }

    #[test]
    fn bad_hours_are_refused() {
        assert!(validate(&sofia(&[(&[1], "08:00", "15:00")])).is_ok());
        assert!(validate(&sofia(&[(&[0], "08:00", "15:00")])).is_err());
        assert!(validate(&sofia(&[(&[1], "8", "15:00")])).is_err());
        assert!(validate(&sofia(&[(&[1], "25:00", "15:00")])).is_err());
        let mut h = sofia(&[]);
        h.timezone = "Mars/Olympus".into();
        assert!(validate(&h).is_err());
    }
}
