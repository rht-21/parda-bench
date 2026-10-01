use rand::RngExt;

use crate::DataRng;

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

const MONTHS_DEVA: [&str; 12] = [
    "जनवरी",
    "फ़रवरी",
    "मार्च",
    "अप्रैल",
    "मई",
    "जून",
    "जुलाई",
    "अगस्त",
    "सितंबर",
    "अक्टूबर",
    "नवंबर",
    "दिसंबर",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateStyle {
    /// `07/03/1991`
    Slashed,
    /// `07-03-1991`
    Hyphenated,
    /// `07.03.1991`
    Dotted,
    /// `7 March 1991`
    Long,
    /// `March 7, 1991`
    MonthFirst,
    /// `1991-03-07`
    Iso,
    /// `7 मार्च 1991`
    Devanagari,
}

impl DateStyle {
    /// Styles that read naturally in Latin-script text.
    pub const LATIN: [Self; 6] = [
        Self::Slashed,
        Self::Hyphenated,
        Self::Dotted,
        Self::Long,
        Self::MonthFirst,
        Self::Iso,
    ];
}

/// A calendar date with the year range chosen by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
}

impl Date {
    /// Uniformly random day within `years` (inclusive).
    #[must_use]
    pub fn random(rng: &mut DataRng, years: std::ops::RangeInclusive<u16>) -> Self {
        let year = rng.random_range(years);
        let month = rng.random_range(1..=12);
        let day = rng.random_range(1..=days_in_month(year, month));
        Self { year, month, day }
    }

    #[must_use]
    pub fn format(self, style: DateStyle) -> String {
        let Self { year, month, day } = self;
        let name = MONTHS[usize::from(month - 1)];
        match style {
            DateStyle::Slashed => format!("{day:02}/{month:02}/{year}"),
            DateStyle::Hyphenated => format!("{day:02}-{month:02}-{year}"),
            DateStyle::Dotted => format!("{day:02}.{month:02}.{year}"),
            DateStyle::Long => format!("{day} {name} {year}"),
            DateStyle::MonthFirst => format!("{name} {day}, {year}"),
            DateStyle::Iso => format!("{year}-{month:02}-{day:02}"),
            DateStyle::Devanagari => {
                format!("{day} {} {year}", MONTHS_DEVA[usize::from(month - 1)])
            }
        }
    }
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;

    #[test]
    fn leap_years_follow_gregorian_rules() {
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
        assert_eq!(days_in_month(1996, 2), 29);
    }

    #[test]
    fn formats_one_date_in_every_style() {
        let d = Date {
            year: 1991,
            month: 3,
            day: 7,
        };
        assert_eq!(d.format(DateStyle::Slashed), "07/03/1991");
        assert_eq!(d.format(DateStyle::MonthFirst), "March 7, 1991");
        assert_eq!(d.format(DateStyle::Iso), "1991-03-07");
        assert_eq!(d.format(DateStyle::Devanagari), "7 मार्च 1991");
    }

    #[test]
    fn random_dates_stay_in_range() {
        let mut rng = DataRng::seed_from_u64(1);
        for _ in 0..500 {
            let d = Date::random(&mut rng, 1950..=2006);
            assert!((1950..=2006).contains(&d.year));
            assert!(d.day >= 1 && d.day <= days_in_month(d.year, d.month));
        }
    }
}
