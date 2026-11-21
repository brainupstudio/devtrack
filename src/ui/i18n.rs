//! Interface language: English (the message ids) and Italian.
//!
//! Messages are identified by their English text (gettext style): `tr("Save")` → "Salva".
//! Text with values uses templates: `trf("{} of {}", &[&a, &b])`; a translation may reorder its
//! values with `{0}`, `{1}`. Counts use `plural(n, "{} session", "{} sessions")`, which picks the
//! form for the language (Italian: 1 = singular, otherwise plural). Numbers stay as they are
//! (Mono), user data (names, titles, codes) is never passed through `tr`. Destination values,
//! decimal separators and date formats belong to each destination and never follow the
//! interface language.

use chrono::{Datelike, NaiveDate, Weekday};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    It,
}

/// The stored preference.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LangPref {
    System,
    En,
    It,
}

impl LangPref {
    pub fn parse(s: &str) -> LangPref {
        match s {
            "en" => LangPref::En,
            "it" => LangPref::It,
            _ => LangPref::System,
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            LangPref::System => "system",
            LangPref::En => "en",
            LangPref::It => "it",
        }
    }
    /// System: macOS preferred languages, cached once; environment locale as a fallback on
    /// other platforms or when preferences cannot be read. Unsupported languages use English.
    pub fn resolve(self) -> Lang {
        match self {
            LangPref::En => Lang::En,
            LangPref::It => Lang::It,
            LangPref::System => system_lang(),
        }
    }
}

pub fn system_lang() -> Lang {
    #[cfg(target_os = "macos")]
    {
        static SYSTEM: OnceLock<Option<Lang>> = OnceLock::new();
        if let Some(l) = *SYSTEM.get_or_init(|| {
            let out = std::process::Command::new("/usr/bin/defaults").args(["read", "-g", "AppleLanguages"]).output().ok()?;
            if !out.status.success() { return None; }
            let text = String::from_utf8(out.stdout).ok()?;
            let first = text.split(|c: char| !c.is_ascii_alphanumeric() && c != '-').find(|s| !s.is_empty())?;
            Some(if first == "it" || first.starts_with("it-") { Lang::It } else { Lang::En })
        }) { return l; }
    }
    for k in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        if let Ok(v) = std::env::var(k) {
            if !v.is_empty() && v != "C" && v != "POSIX" {
                return if v.to_lowercase().starts_with("it") { Lang::It } else { Lang::En };
            }
        }
    }
    Lang::En
}

static LANG: AtomicU8 = AtomicU8::new(0);

pub fn set_lang(l: Lang) {
    LANG.store(if l == Lang::It { 1 } else { 0 }, Ordering::Relaxed);
}

pub fn lang() -> Lang {
    if LANG.load(Ordering::Relaxed) == 1 {
        Lang::It
    } else {
        Lang::En
    }
}

fn table() -> &'static HashMap<&'static str, &'static str> {
    static T: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    T.get_or_init(|| super::i18n_it::IT.iter().cloned().collect())
}

/// Translate one message (its English text is the id).
pub fn tr(s: &str) -> String {
    match lang() {
        Lang::En => s.to_string(),
        Lang::It => table().get(s).map(|t| t.to_string()).unwrap_or_else(|| s.to_string()),
    }
}

/// Whether a message has an Italian translation (used by the completeness test).
pub fn has_it(s: &str) -> bool {
    table().contains_key(s)
}

fn fill(template: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = template.to_string();
    for (i, a) in args.iter().enumerate() {
        let pos = format!("{{{}}}", i);
        if out.contains(&pos) {
            out = out.replace(&pos, &a.to_string());
        } else if let Some(at) = out.find("{}") {
            out.replace_range(at..at + 2, &a.to_string());
        }
    }
    out
}

/// Translate a template, then put the values in.
pub fn trf(template: &str, args: &[&dyn std::fmt::Display]) -> String {
    fill(&tr(template), args)
}

/// Count with the right form: `plural(3, "{} session", "{} sessions")`.
pub fn plural(n: i64, one: &str, many: &str) -> String {
    let t = if n == 1 { one } else { many };
    fill(&tr(t), &[&n])
}

pub fn weekday(d: NaiveDate) -> String {
    let i = d.weekday().num_days_from_monday() as usize;
    match lang() {
        Lang::En => ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"][i].into(),
        Lang::It => ["lunedì", "martedì", "mercoledì", "giovedì", "venerdì", "sabato", "domenica"][i].into(),
    }
}

pub fn weekday_short(d: NaiveDate) -> String {
    let i = d.weekday().num_days_from_monday() as usize;
    match lang() {
        Lang::En => ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][i].into(),
        Lang::It => ["lun", "mar", "mer", "gio", "ven", "sab", "dom"][i].into(),
    }
}

pub fn month(m: u32) -> String {
    let i = (m.clamp(1, 12) - 1) as usize;
    match lang() {
        Lang::En => ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"][i].into(),
        Lang::It => ["gennaio", "febbraio", "marzo", "aprile", "maggio", "giugno", "luglio", "agosto", "settembre", "ottobre", "novembre", "dicembre"][i].into(),
    }
}

/// Numeric date for display: ISO in English, day/month/year in Italian ("18/11/2026"). Typed
/// date fields keep YYYY-MM-DD; this is only for reading.
pub fn short_date(d: NaiveDate) -> String {
    match lang() {
        Lang::En => d.format("%Y-%m-%d").to_string(),
        Lang::It => d.format("%d/%m/%Y").to_string(),
    }
}

/// "Tuesday, November 17, 2026" / "martedì 17 novembre 2026".
pub fn long_date(d: NaiveDate) -> String {
    match lang() {
        Lang::En => format!("{}, {} {}, {}", weekday(d), month(d.month()), d.day(), d.year()),
        Lang::It => format!("{} {} {} {}", weekday(d), d.day(), month(d.month()), d.year()),
    }
}

/// Capitalised weekday for headings ("Lunedì").
pub fn weekday_title(d: NaiveDate) -> String {
    let w = weekday(d);
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => w,
    }
}

#[allow(dead_code)]
pub fn is_weekend(d: NaiveDate) -> bool {
    matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_plurals_and_dates() {
        set_lang(Lang::En);
        assert_eq!(plural(1, "{} session", "{} sessions"), "1 session");
        assert_eq!(plural(3, "{} session", "{} sessions"), "3 sessions");
        assert_eq!(trf("{} of {}", &[&2, &5]), "2 of 5");
        let d = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        assert_eq!(long_date(d), "Tuesday, November 17, 2026");
        set_lang(Lang::It);
        assert_eq!(tr("Save"), "Salva");
        assert_eq!(tr("Cancel"), "Annulla");
        assert_eq!(plural(1, "{} session", "{} sessions"), "1 sessione");
        assert_eq!(plural(3, "{} session", "{} sessions"), "3 sessioni");
        assert_eq!(long_date(d), "martedì 17 novembre 2026");
        // user data that isn't a message passes through unchanged
        assert_eq!(tr("NW-PORTAL"), "NW-PORTAL");
        set_lang(Lang::En);
    }

    #[test]
    fn italian_table_has_no_conflicting_entries() {
        let mut seen: HashMap<&str, &str> = HashMap::new();
        for (k, v) in super::super::i18n_it::IT {
            if let Some(old) = seen.insert(k, v) {
                assert_eq!(old, *v, "two translations for {:?}", k);
            }
            // every value keeps the same placeholders as its message
            assert_eq!(k.matches("{}").count(), v.matches("{}").count(), "placeholders differ for {:?}", k);
        }
    }

    #[test]
    fn unsupported_system_language_falls_back_to_english() {
        assert_eq!(LangPref::En.resolve(), Lang::En);
        assert_eq!(LangPref::It.resolve(), Lang::It);
        assert_eq!(LangPref::parse("fr"), LangPref::System);
    }
}

/// `trf_m!("{} of {}", a, b)`: like `format!`, but the template is translated first.
macro_rules! trf_m {
    ($t:expr $(, $a:expr)* $(,)?) => {
        $crate::ui::i18n::trf($t, &[$(&$a as &dyn std::fmt::Display),*])
    };
}
pub(crate) use trf_m;
