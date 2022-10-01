// -----------------------------------------------------------------------------------------------
// Coaly - context aware logging and tracing system
//
// Copyright (c) 2022, Frank Sommer.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// * Redistributions of source code must retain the above copyright notice, this
//   list of conditions and the following disclaimer.
//
// * Redistributions in binary form must reproduce the above copyright notice,
//   this list of conditions and the following disclaimer in the documentation
//   and/or other materials provided with the distribution.
//
// * Neither the name of the copyright holder nor the names of its
//   contributors may be used to endorse or promote products derived from
//   this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
// -----------------------------------------------------------------------------------------------

//! Coaly date-time types.

use chrono::*;
use regex::Regex;
use std::cmp::min;
use std::fmt::{Debug, Formatter};
use std::str::FromStr;
use crate::errorhandling::*;
use crate::{CoalyResult, coalyxw};

/// Maximum time span allowed (e.g. for file rollover).
/// We're using one (leap) year.
pub(crate) const MAX_DURATION: i64 = 366*24*3600;

/// Time span units
#[derive (Clone, Copy, PartialEq)]
pub(crate) enum TimeSpanUnit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month
}
impl Debug for TimeSpanUnit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeSpanUnit::Second => write!(f, "{}", TS_UNIT_SEC),
            TimeSpanUnit::Minute => write!(f, "{}", TS_UNIT_MIN),
            TimeSpanUnit::Hour => write!(f, "{}", TS_UNIT_HOUR),
            TimeSpanUnit::Day => write!(f, "{}", TS_UNIT_DAY),
            TimeSpanUnit::Week => write!(f, "{}", TS_UNIT_WEEK),
            TimeSpanUnit::Month => write!(f, "{}", TS_UNIT_MONTH),
        }
    }
}
impl FromStr for TimeSpanUnit {
    type Err = bool;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            TS_UNIT_SEC | TS_UNIT_SECS => Ok(TimeSpanUnit::Second),
            TS_UNIT_MIN | TS_UNIT_MINS => Ok(TimeSpanUnit::Minute),
            TS_UNIT_HOUR | TS_UNIT_HOURS => Ok(TimeSpanUnit::Hour),
            TS_UNIT_DAY | TS_UNIT_DAYS => Ok(TimeSpanUnit::Day),
            TS_UNIT_WEEK| TS_UNIT_WEEKS => Ok(TimeSpanUnit::Week),
            TS_UNIT_MONTH | TS_UNIT_MONTHS => Ok(TimeSpanUnit::Month),
            _ => Err(false)
        }
    }
}

/// Weekdays
#[derive (Clone, Copy)]
#[repr(u8)]
pub(crate) enum WeekDay {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday
}
impl Debug for WeekDay {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WeekDay::Monday => write!(f, "{}", WEEKDAY_MONDAY),
            WeekDay::Tuesday => write!(f, "{}", WEEKDAY_TUESDAY),
            WeekDay::Wednesday => write!(f, "{}", WEEKDAY_WEDNESDAY),
            WeekDay::Thursday => write!(f, "{}", WEEKDAY_THURSDAY),
            WeekDay::Friday => write!(f, "{}", WEEKDAY_FRIDAY),
            WeekDay::Saturday => write!(f, "{}", WEEKDAY_SATURDAY),
            WeekDay::Sunday => write!(f, "{}", WEEKDAY_SUNDAY),
        }
    }
}
impl FromStr for WeekDay {
    type Err = bool;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            WEEKDAY_MONDAY => Ok(WeekDay::Monday),
            WEEKDAY_TUESDAY => Ok(WeekDay::Tuesday),
            WEEKDAY_WEDNESDAY => Ok(WeekDay::Wednesday),
            WEEKDAY_THURSDAY => Ok(WeekDay::Thursday),
            WEEKDAY_FRIDAY => Ok(WeekDay::Friday),
            WEEKDAY_SATURDAY => Ok(WeekDay::Saturday),
            WEEKDAY_SUNDAY => Ok(WeekDay::Sunday),
            _ => Err(false)
        }
    }
}

/// Time span, e.g. 5 hours.
#[derive (Clone)]
pub(crate) struct TimeSpan {
    unit: TimeSpanUnit,
    value: i64
}
impl TimeSpan {
    /// Creates a time span.
    ///
    /// # Arguments
    /// * `unit` - the time span unit (seconds, hours, ...)
    /// * `value` - the number of time span units
    pub(crate) fn new (unit: TimeSpanUnit, value: u32) -> TimeSpan {
        TimeSpan { unit, value: value as i64 }
    }

    /// Returns the duration of the time span in seconds.
    ///
    /// # Arguments
    /// * `from` - the date and time when the duration shall start
    pub(crate) fn duration (&self, from: &DateTime<Local>) -> i64 {
        let duration_secs = match self.unit {
            TimeSpanUnit::Second => self.value,
            TimeSpanUnit::Minute => self.value * 60,
            TimeSpanUnit::Hour => self.value * 3600,
            TimeSpanUnit::Day => self.value * 86400,
            TimeSpanUnit::Week => self.value * 604800,
            TimeSpanUnit::Month => {
                let mut year = from.date().year();
                let mut month = from.date().month();
                let day = from.date().day();
                // all remaining days of first month
                let mut day_count = days_in_month(month, year) - day;
                if month < 12 { month += 1; } else { month = 1; year += 1; }
                // all days of intermediate months
                for _i in 1..self.value {
                    day_count += days_in_month(month, year);
                    if month < 12 { month += 1; } else { month = 1; year += 1; }
                }
                // days up to start day of last month
                let days_in_last = days_in_month(month, year);
                day_count += min(days_in_last, day);
                (day_count as i64) * 86400
            }
        };
        i64::min(duration_secs, MAX_DURATION)
    }    
}
impl Debug for TimeSpan {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "U:{:?}/V:{}", self.unit, self.value)
    }
}

/// Specific anchor moment, when a time span elapses.
/// The moment depends on the unit of the time span, e.g. for a time span measured in days
/// the moment is hour and minute.
/// Corresponds to the value following the at in a file rollover specification.
#[derive (Clone)]
pub(crate) struct TimeStampAnchor {
    minute: u32,
    hour: u32,
    day_of_week: u32,
    day_of_month: u32
}
impl TimeStampAnchor {
    /// Creates an anchor for a time span specified in hours, e.g. "every hour at 15"
    ///
    /// # Arguments
    /// * `minute` - the minute within the hour (0 - 59)
    #[inline]
    pub(crate) fn for_unit_hour(minute: u32) -> TimeStampAnchor {
        TimeStampAnchor { minute, hour: 0, day_of_week: 0, day_of_month: 0 }
    }

    /// Creates an anchor for a time span specified in days, e.g. "every day at 22:00"
    ///
    /// # Arguments
    /// * `hour` - the hour (0 - 23)
    /// * `minute` - the minute within the hour (0 - 59)
    #[inline]
    pub(crate) fn for_unit_day(hour: u32, minute: u32) -> TimeStampAnchor {
        TimeStampAnchor { minute, hour, day_of_week: 0, day_of_month: 0 }
    }

    /// Creates an anchor for a time span specified in weeks,
    /// e.g. "every 2 weeks at sunday 03:00"
    ///
    /// # Arguments
    /// * `day_of_week` - the weekday (Monday - Sunday)
    /// * `hour` - the hour (0 - 23)
    /// * `minute` - the minute within the hour (0 - 59)
    #[inline]
    pub(crate) fn for_unit_week(day_of_week: WeekDay, hour: u32, minute: u32) -> TimeStampAnchor {
        TimeStampAnchor { minute, hour, day_of_week: day_of_week as u32, day_of_month: 0 }
    }

    /// Creates an anchor for a time span specified in months,
    /// e.g. "every month at 15 03:00"
    ///
    /// # Arguments
    /// * `day_of_month` - the day within the month, a value larger than the month has days is
    ///                    considered as ultimo
    /// * `hour` - the hour
    /// * `minute` - the minute within the hour
    #[inline]
    pub(crate) fn for_unit_month(day_of_month: u32, hour: u32, minute: u32) -> TimeStampAnchor {
        TimeStampAnchor { minute, hour, day_of_week: 0, day_of_month }
    }

    /// Returns the timespan anchor specification for the given unit.
    /// An anchor defines the moment, when a timespan interval starts. Allowed combinations are:
    /// * Unit hour -> 00 ... 59 (minute)
    /// * Unit day -> 00:00 ... 23:59 (hour and minute)
    /// * Unit week -> monday 00:00 ... sunday 23:59 (day of week, hour and minute)
    /// * Unit month -> 01 00:00 ... 31 23:59 (day, hour and minute)
    /// 
    /// # Arguments
    /// * `anchor_str` - the anchor string specification, lowercase
    /// * `unit` - the time span unit (allowed are hour, day, week or month)
    /// 
    /// # Return values
    /// The time span anchor, if the specification is valid; otherwise Exception
    pub(crate) fn for_unit(anchor_str: &str,
                    unit: &TimeSpanUnit) -> CoalyResult<TimeStampAnchor> {
        match unit {
            TimeSpanUnit::Hour => {
                if let Ok(h) = u32::from_str(anchor_str) {
                    if (0..=59).contains(&h) { return Ok(TimeStampAnchor::for_unit_hour(h)) }
                }
                Err(coalyxw!(W_CFG_ANCHOR_MIN_REQ, anchor_str.to_string()))
            },
            TimeSpanUnit::Day => {
                let pattern = Regex::new(ANCHOR_HOUR_PATTERN).unwrap();
                if let Some(capts) = pattern.captures(anchor_str) {
                    let hour = u32::from_str(capts.get(1).unwrap().as_str());
                    let min = u32::from_str(capts.get(2).unwrap().as_str());
                    if hour.is_err() || min.is_err() {
                        return Err(coalyxw!(W_CFG_ANCHOR_HHMM_REQ, anchor_str.to_string()))
                    }
                    let hour = hour.unwrap();
                    let min = min.unwrap();
                    if (0..=23).contains(&hour) && (0..=59).contains(&min) {
                        return Ok(TimeStampAnchor::for_unit_day(hour, min))
                    }
                }
                Err(coalyxw!(W_CFG_ANCHOR_HHMM_REQ, anchor_str.to_string()))
            },
            TimeSpanUnit::Week => {
                let pattern = Regex::new(ANCHOR_DOW_PATTERN).unwrap();
                if let Some(capts) = pattern.captures(anchor_str) {
                    let dow = WeekDay::from_str(capts.get(1).unwrap().as_str());
                    let hour = u32::from_str(capts.get(2).unwrap().as_str());
                    let min = u32::from_str(capts.get(3).unwrap().as_str());
                    if dow.is_err() || hour.is_err() || min.is_err() {
                        return Err(coalyxw!(W_CFG_ANCHOR_DOWHM_REQ, anchor_str.to_string()))
                    }
                    let dow = dow.unwrap();
                    let hour = hour.unwrap();
                    let min = min.unwrap();
                    if (0..=23).contains(&hour) && (0..=59).contains(&min) {
                        return Ok(TimeStampAnchor::for_unit_week(dow, hour, min))
                    }
                }
                Err(coalyxw!(W_CFG_ANCHOR_DOWHM_REQ, anchor_str.to_string()))
            },
            TimeSpanUnit::Month => {
                let pattern = Regex::new(ANCHOR_DOM_PATTERN).unwrap();
                if let Some(capts) = pattern.captures(anchor_str) {
                    let dom_spec = capts.get(1).unwrap().as_str();
                    let dom = if dom_spec == "ultimo" { Ok(31) } else { u32::from_str(dom_spec) };
                    let hour = u32::from_str(capts.get(2).unwrap().as_str());
                    let min = u32::from_str(capts.get(3).unwrap().as_str());
                    if dom.is_err() || hour.is_err() || min.is_err() {
                        return Err(coalyxw!(W_CFG_ANCHOR_DOMHM_REQ, anchor_str.to_string()))
                    }
                    let mut dom = dom.unwrap();
                    let hour = hour.unwrap();
                    let min = min.unwrap();
                    if dom > 31 { dom = 31; }
                    if (0..=23).contains(&hour) && (0..=59).contains(&min) && dom > 0 {
                        return Ok(TimeStampAnchor::for_unit_month(dom, hour, min))
                    }
                }
                Err(coalyxw!(W_CFG_ANCHOR_DOMHM_REQ, anchor_str.to_string()))
            },
            _ => Err(coalyxw!(W_CFG_ANCHOR_NOT_ALLOWED, anchor_str.to_string()))
        }
    }
}
impl Debug for TimeStampAnchor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "MD:{}/WD:{}/HR:{}/MI:{}",
               self.day_of_month, self.day_of_week, self.hour, self.minute)
    }
}

/// Time interval
#[derive (Clone)]
pub(crate) struct Interval {
    // interval duration, e.g. 2 weeks
    time_span: TimeSpan,
    // optional an anchor moment, e.g. wednesday at 15:00
    anchor: Option<TimeStampAnchor>
}
impl Interval {
    /// Creates an interval elapsing at a certain moment.
    /// e.g. "2 weeks at sunday 03:00"
    ///
    /// # Arguments
    /// * `time_span` - the time span
    /// * `anchor` - the specific anchor moment, when the time span elapses
    #[inline]
    pub(crate) fn anchored(time_span: TimeSpan, anchor: TimeStampAnchor) -> Interval {
        Interval { time_span, anchor: Some(anchor) }
    }

    /// Creates an interval elapsing regularly, e.g. every 2 days.
    /// The first interval starts together with the application.
    ///
    /// # Arguments
    /// * `time_span` - the time span
    #[inline]
    pub(crate) fn unanchored(time_span: TimeSpan) -> Interval {
        Interval { time_span, anchor: None }
    }

    /// Returns the timestamp when this interval will elapse.
    ///
    /// # Arguments
    /// * `last_elapsed` - the timestamp when the interval elapsed last
    pub(crate) fn next_elapse(&self,
                       last_elapsed: &DateTime<Local>) -> DateTime<Local> {
        let duration = self.time_span.duration(last_elapsed);
        // duration is limited to maximum of one year, so we can safely ignore an overflow and
        // unwrap the result from checked_add_signed
        let next = last_elapsed.checked_add_signed(Duration::seconds(duration)).unwrap();
        if self.anchor.is_some() {
            let anchored_next = self.next_match(&next);
            let diff_to_next = anchored_next.timestamp() - next.timestamp();
            if  diff_to_next == 0 { return next }
            let anchored_prev = self.prev_match(&next);
            let diff_to_prev = next.timestamp() - anchored_prev.timestamp();
            return if diff_to_next < diff_to_prev { anchored_next } else { anchored_prev }
        }
        next
    }

    /// Determines the timestamp equal or later than the specified instant under consideration of
    /// the interval's anchor.
    ///
    /// # Arguments
    /// * `instant` - the timestamp when the interval should elapse without considering the anchor
   fn next_match(&self, instant: &DateTime<Local>) -> DateTime<Local> {
        if let Some(a) = &self.anchor {
            let delta = if a.minute < instant.minute() { a.minute + 60 - instant.minute() }
                        else { a.minute - instant.minute() };
            let mut res = instant.checked_add_signed(Duration::minutes(delta as i64)).unwrap();
            if self.time_span.unit == TimeSpanUnit::Hour { return res }
            let delta = if a.hour < res.hour() { a.hour + 24 - res.hour() }
                        else { a.hour - res.hour() };
            res = res.checked_add_signed(Duration::hours(delta as i64)).unwrap();
            if self.time_span.unit == TimeSpanUnit::Day { return res }
            if self.time_span.unit == TimeSpanUnit::Week {
                let iwd = res.weekday().num_days_from_monday();
                let delta = if a.day_of_week < iwd { a.day_of_week + 7 - iwd }
                            else { a.day_of_week - iwd };
                return res.checked_add_signed(Duration::days(delta as i64)).unwrap()
            }
            // span month
            let days_curr_month = days_in_month(res.month(), res.year());
            let aday = std::cmp::min(a.day_of_month, days_curr_month);
            if res.day() <= aday {
                let delta = aday - res.day();
                return res.checked_add_signed(Duration::days(delta as i64)).unwrap()
            }
            let delta = days_curr_month - res.day();
            let next_month = if res.month() < 12 { res.month() + 1 } else { 1 };
            let next_month_y = if res.month() < 12 { res.year() } else { res.year() + 1 };
            let days_next_month = days_in_month(next_month, next_month_y);
            let aday = std::cmp::min(a.day_of_month, days_next_month);
            return res.checked_add_signed(Duration::days((delta + aday) as i64)).unwrap()
        }
        *instant
    }

    /// Determines the instant equal or sooner as the specified instant under consideration of
    /// the interval's anchor.
    ///
    /// # Arguments
    /// * `instant` - the timestamp when the interval should elapse without considering the anchor
    fn prev_match(&self, instant: &DateTime<Local>) -> DateTime<Local> {
        if let Some(a) = &self.anchor {
            let delta = if a.minute < instant.minute() { instant.minute() - a.minute }
                        else { instant.minute() + 60 - a.minute };
            let mut res = instant.checked_sub_signed(Duration::minutes(delta as i64)).unwrap();
            if self.time_span.unit == TimeSpanUnit::Hour { return res }
            let delta = if a.hour < res.hour() { res.hour() - a.hour }
                        else { res.hour() + 24 - a.hour };
            res = res.checked_sub_signed(Duration::hours(delta as i64)).unwrap();
            if self.time_span.unit == TimeSpanUnit::Day { return res }
            if self.time_span.unit == TimeSpanUnit::Week {
                let iwd = res.weekday().num_days_from_monday();
                let delta = if a.day_of_week <= iwd { iwd - a.day_of_week }
                            else { iwd + 7 - a.day_of_week};
                return res.checked_sub_signed(Duration::days(delta as i64)).unwrap()
            }
            // change span month
            let days_curr_month = days_in_month(res.month(), res.year());
            let aday = std::cmp::min(a.day_of_month, days_curr_month);
            if res.day() >= aday {
                let delta = res.day() - aday;
                return res.checked_sub_signed(Duration::days(delta as i64)).unwrap()
            }
            let prev_month = if res.month() > 1 { res.month() - 1 } else { 12 };
            let prev_month_y = if res.month() > 1 { res.year() } else { res.year() - 1 };
            let days_prev_month = days_in_month(prev_month, prev_month_y);
            let aday = std::cmp::min(a.day_of_month, days_prev_month);
            let delta = res.day() + days_prev_month - aday;
            return res.checked_sub_signed(Duration::days((delta) as i64)).unwrap()
        }
        *instant
    }
}
impl Debug for Interval {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(a) = &self.anchor { return write!(f, "TS:{:?}/A:{:?}", self.time_span, a) }
        write!(f, "TS:{:?}/A:-", self.time_span)
    }
}

// Names for all weekdays
const WEEKDAY_MONDAY: &str = "monday";
const WEEKDAY_TUESDAY: &str = "tuesday";
const WEEKDAY_WEDNESDAY: &str = "wednesday";
const WEEKDAY_THURSDAY: &str = "thursday";
const WEEKDAY_FRIDAY: &str = "friday";
const WEEKDAY_SATURDAY: &str = "saturday";
const WEEKDAY_SUNDAY: &str = "sunday";

// Names for all time span units
const TS_UNIT_SEC: &str = "second";
const TS_UNIT_MIN: &str = "minute";
const TS_UNIT_HOUR: &str = "hour";
const TS_UNIT_DAY: &str = "day";
const TS_UNIT_WEEK: &str = "week";
const TS_UNIT_MONTH: &str = "month";
const TS_UNIT_SECS: &str = "seconds";
const TS_UNIT_MINS: &str = "minutes";
const TS_UNIT_HOURS: &str = "hours";
const TS_UNIT_DAYS: &str = "days";
const TS_UNIT_WEEKS: &str = "weeks";
const TS_UNIT_MONTHS: &str = "months";

// Regular expression patterns to parse date/time specifications
const ANCHOR_HOUR_PATTERN: &str = "^([0-9]{2}):([0-9]{2})$";
const ANCHOR_DOW_PATTERN: &str = "^([a-z]+)\\s+([0-9]{2}):([0-9]{2})$";
const ANCHOR_DOM_PATTERN: &str = "^([0-9]{2}|ultimo)\\s+([0-9]{2}):([0-9]{2})$";

/// Returns the number of days in the specified month/year combination.
///
/// # Arguments
/// * `month` - the month with 1=January and 12=December
/// * `year` - the year, needed to handle leap years
fn days_in_month(month: u32, year: i32) -> u32 {
    match month {
        2 => {
            if year % 400 == 0 { return 29 }
            if year % 100 == 0 { return 28 }
            if year % 4 == 0 { 29 } else { 28 }
        },
        4 | 6 | 9 | 11 => 30,
        _ => 31
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONTH_DAYS: &[(u32, i32, u32)] =
        &[
            (1, 2021, 31),
            (2, 2021, 28),
            (2, 2020, 29),
            (2, 2000, 29),
            (2, 1900, 28),
            (3, 2021, 31),
            (4, 2021, 30),
            (5, 2021, 31),
            (6, 2021, 30),
            (7, 2021, 31),
            (8, 2021, 31),
            (9, 2021, 30),
            (10, 2021, 31),
            (11, 2021, 30),
            (12, 2021, 31)
         ];

    // Hourly time spans, anchor is at XX:15
    const NEXT_ELAPSES_HOURLY: &[(&str, &str, &str)] =
        &[
            ("hourly less_half", "2021-06-15 15:55", "2021-06-15 17:15"),
            ("hourly greater_half", "2021-06-15 15:20", "2021-06-15 16:15"),
            ("hourly exact_half", "2021-06-15 15:45", "2021-06-15 16:15"),
            ("hourly day wrap", "2021-06-15 23:55", "2021-06-16 01:15")
         ];

    // Daily time spans, anchor is at 03:30
    const NEXT_ELAPSES_DAILY: &[(&str, &str, &str)] =
        &[
            ("daily less_half", "2021-06-15 22:00", "2021-06-17 03:30"),
            ("daily greater_half", "2021-06-15 08:00", "2021-06-16 03:30"),
            ("daily exact_half", "2021-06-15 15:30", "2021-06-16 03:30"),
            ("daily month wrap", "2021-06-30 11:00", "2021-07-01 03:30"),
            ("daily feb28 leap", "2020-02-28 10:00", "2020-02-29 03:30"),
            ("daily feb28 no leap", "2021-02-28 10:00", "2021-03-01 03:30"),
            ("daily feb29", "2020-02-29 10:00", "2020-03-01 03:30")
         ];

    // Daily time spans, anchor is Wednesday at 12:00
    const NEXT_ELAPSES_WEEKLY: &[(&str, &str, &str)] =
        &[
            ("weekly less_half", "2021-06-20 18:00", "2021-06-30 12:00"),
            ("weekly greater_half", "2021-06-25 15:00", "2021-06-30 12:00"),
            ("weekly exact_half", "2021-06-27 00:00", "2021-06-30 12:00"),
            ("weekly month wrap", "2021-06-30 15:00", "2021-07-07 12:00"),
            ("weekly feb28 leap", "2020-02-28 10:00", "2020-03-04 12:00"),
            ("weekly feb28 no leap", "2021-02-28 20:00", "2021-03-10 12:00"),
            ("weekly feb29", "2020-02-29 10:00", "2020-03-04 12:00")
         ];

    // Daily time spans, anchor is ultimo at 22:00
    const NEXT_ELAPSES_MONTHLY: &[(&str, &str, &str)] =
        &[
            ("monthly less_half", "2021-06-20 18:00", "2021-07-31 22:00"),
            ("monthly greater_half", "2021-06-11 15:00", "2021-06-30 22:00"),
            ("monthly exact_half", "2021-06-15 11:00", "2021-06-30 22:00"),
            ("monthly year wrap", "2020-12-24 17:00", "2021-01-31 22:00"),
            ("monthly feb28 leap", "2020-01-30 10:00", "2020-02-29 22:00"),
            ("monthly feb28 no leap", "2021-01-30 20:00", "2021-02-28 22:00"),
            ("monthly feb29", "2020-02-29 10:00", "2020-03-31 22:00")
         ];

    fn test_interval_elapse(interval: &Interval, test_data: &[(&str, &str, &str)]) {
        for (name, start, exp_next) in test_data {
            let dstart = Local.datetime_from_str(start, "%Y-%m-%d %H:%M").unwrap();
            let dexp_next = Local.datetime_from_str(exp_next, "%Y-%m-%d %H:%M").unwrap();
            let res = interval.next_elapse(&dstart);
            assert_eq!(dexp_next, res, "{}", name);
        }
    }

    #[test]
    fn test_days_in_month() {
        for (month, year, expected_days) in MONTH_DAYS {
            let actual_days = days_in_month(*month, *year);
            assert_eq!(*expected_days, actual_days);
        }
    }

    #[test]
    fn test_timespan_duration() {
        let start_jun15 = Local.datetime_from_str("2021-06-15 16:00", "%Y-%m-%d %H:%M").unwrap();
        let start_jan31_ly = Local.datetime_from_str("2020-01-31 12:00", "%Y-%m-%d %H:%M").unwrap();
        let start_jan31_noly = Local.datetime_from_str("2021-01-31 12:00", "%Y-%m-%d %H:%M").unwrap();
        let start_feb29 = Local.datetime_from_str("2020-02-29 12:00", "%Y-%m-%d %H:%M").unwrap();
        let secs_30 = TimeSpan::new(TimeSpanUnit::Second, 30);
        let mins_15 = TimeSpan::new(TimeSpanUnit::Minute, 15);
        let hours_1 = TimeSpan::new(TimeSpanUnit::Hour, 1);
        let days_3 = TimeSpan::new(TimeSpanUnit::Day, 3);
        let weeks_2 = TimeSpan::new(TimeSpanUnit::Week, 2);
        let months_1 = TimeSpan::new(TimeSpanUnit::Month, 1);
        let months_2 = TimeSpan::new(TimeSpanUnit::Month, 2);
        let months_3 = TimeSpan::new(TimeSpanUnit::Month, 3);
        let months_12 = TimeSpan::new(TimeSpanUnit::Month, 12);
        assert_eq!(30, secs_30.duration(&start_jun15));
        assert_eq!(15*60, mins_15.duration(&start_jun15));
        assert_eq!(60*60, hours_1.duration(&start_jun15));
        assert_eq!(3*24*60*60, days_3.duration(&start_jun15));
        assert_eq!(2*7*24*60*60, weeks_2.duration(&start_jun15));
        assert_eq!(30*24*60*60, months_1.duration(&start_jun15));
        assert_eq!((30+31)*24*60*60, months_2.duration(&start_jun15));
        assert_eq!((30+31+31)*24*60*60, months_3.duration(&start_jun15));
        assert_eq!(365*24*60*60, months_12.duration(&start_jun15));
        assert_eq!(29*24*60*60, months_1.duration(&start_jan31_ly));
        assert_eq!((29+31)*24*60*60, months_2.duration(&start_jan31_ly));
        assert_eq!((29+31+30)*24*60*60, months_3.duration(&start_jan31_ly));
        assert_eq!(366*24*60*60, months_12.duration(&start_jan31_ly));
        assert_eq!(28*24*60*60, months_1.duration(&start_jan31_noly));
        assert_eq!((28+31)*24*60*60, months_2.duration(&start_jan31_noly));
        assert_eq!((28+31+30)*24*60*60, months_3.duration(&start_jan31_noly));
        assert_eq!(365*24*60*60, months_12.duration(&start_jan31_noly));
        assert_eq!(29*24*60*60, months_1.duration(&start_feb29));
        assert_eq!((29+31)*24*60*60, months_2.duration(&start_feb29));
        assert_eq!((29+31+30)*24*60*60, months_3.duration(&start_feb29));
        assert_eq!(365*24*60*60, months_12.duration(&start_feb29));
    }

    #[test]
    fn test_next_elapse() {
        // HOURLY
        let span_hrs_1 = TimeSpan::new(TimeSpanUnit::Hour, 1);
        let anchor_min_15 = TimeStampAnchor::for_unit_hour(15);
        let interval = Interval::anchored(span_hrs_1, anchor_min_15);
        test_interval_elapse(&interval, NEXT_ELAPSES_HOURLY);
        // DAILY
        let span_days_1 = TimeSpan::new(TimeSpanUnit::Day, 1);
        let anchor_hm_3_30 = TimeStampAnchor::for_unit_day(3, 30);
        let interval = Interval::anchored(span_days_1, anchor_hm_3_30);
        test_interval_elapse(&interval, NEXT_ELAPSES_DAILY);
        // WEEKLY
        let span_weeks_1 = TimeSpan::new(TimeSpanUnit::Week, 1);
        let anchor_wd_wed_hm_1200 = TimeStampAnchor::for_unit_week(WeekDay::Wednesday, 12, 0);
        let interval = Interval::anchored(span_weeks_1, anchor_wd_wed_hm_1200);
        test_interval_elapse(&interval, NEXT_ELAPSES_WEEKLY);
        // MONTHLY
        let span_months_1 = TimeSpan::new(TimeSpanUnit::Month, 1);
        let anchor_ult_2200 = TimeStampAnchor::for_unit_month(32, 22, 0);
        let interval = Interval::anchored(span_months_1, anchor_ult_2200);
        test_interval_elapse(&interval, NEXT_ELAPSES_MONTHLY);
    }
}
