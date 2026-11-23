use std::sync::OnceLock;

pub fn portuguese() -> bool {
    static PORTUGUESE: OnceLock<bool> = OnceLock::new();
    *PORTUGUESE.get_or_init(|| {
        gtk::glib::language_names()
            .iter()
            .next()
            .is_some_and(|name| name.split(['_', '-', '.', '@']).next() == Some("pt"))
    })
}

macro_rules! tr {
    ($pt:literal, $en:literal) => {
        if crate::i18n::portuguese() {
            $pt
        } else {
            $en
        }
    };
}

macro_rules! trf {
    ($pt:literal, $en:literal $(, $argument:expr)* $(,)?) => {
        if crate::i18n::portuguese() { format!($pt $(, $argument)*) }
        else { format!($en $(, $argument)*) }
    };
}
