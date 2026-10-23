//! The language in use and the catalogs of a component (docs/i18n.md). The page's `__cui_locale_init`, `__cui_t` and `__cui_locale_set`
//! are this in JavaScript; `tests/i18n_corpus.json` and `tests/i18n_pages.json` are checked against both.

use cancho_ui_ir::i18n;
use cancho_ui_ir::ir::{IrCatalog, IrComponent, IrMessage, MsgPart};

#[derive(Debug, Clone)]
pub struct LocaleState {
    pub lang: String,
    pub langs: Vec<String>,
    pub catalogs: Vec<IrCatalog>,
}

impl LocaleState {
    /// The languages of the component's catalogs, the first in use; English when it has none.
    pub fn new(comp: &IrComponent) -> LocaleState {
        let langs: Vec<String> = if comp.messages.is_empty() {
            vec!["en".to_string()]
        } else {
            comp.messages.iter().map(|m| m.lang.clone()).collect()
        };
        LocaleState {
            lang: langs[0].clone(),
            langs,
            catalogs: comp.messages.clone(),
        }
    }

    /// The first of the browser's preferred languages whose primary subtag the component has; else the first language.
    pub fn choose(&mut self, prefs: &[String]) {
        for p in prefs {
            let primary = p
                .to_ascii_lowercase()
                .split(['-', '_'])
                .next()
                .unwrap_or("")
                .to_string();
            if self.langs.contains(&primary) {
                self.lang = primary;
                return;
            }
        }
        self.lang = self.langs[0].clone();
    }

    pub fn dir(&self) -> &'static str {
        if i18n::lang(&self.lang).is_some_and(|l| l.rtl) {
            "rtl"
        } else {
            "ltr"
        }
    }

    pub fn data(&self) -> &'static i18n::Lang {
        // every language of a component was checked against the tables when it was lowered; "en" is in them
        i18n::lang(&self.lang)
            .or_else(|| i18n::lang("en"))
            .unwrap_or(&i18n::LANGS[0])
    }

    /// `locale.set(lang)`: true when the component has the language.
    pub fn set(&mut self, lang: &str) -> bool {
        if self.langs.iter().any(|l| l == lang) {
            self.lang = lang.to_string();
            true
        } else {
            false
        }
    }

    fn message(&self, key: &str) -> Option<&IrMessage> {
        self.catalogs
            .iter()
            .find(|c| c.lang == self.lang)?
            .entries
            .iter()
            .find(|m| m.key == key)
    }

    /// The text of `key` in the language in use. `args` are the texts of the holes, in the order of the default language's first
    /// appearance of each (a plural message takes its count's plural category as `plural` and the text of the count as the only hole).
    pub fn text(&self, key: &str, args: &[String], count: Option<f64>) -> String {
        let Some(m) = self.message(key) else {
            return String::new();
        };
        let order = self
            .catalogs
            .first()
            .and_then(|c| c.entries.iter().find(|e| e.key == key))
            .map(cancho_ui_ir::messages::hole_order)
            .unwrap_or_default();
        let form = match (&m.plural, count) {
            (Some(_), Some(n)) => {
                let cat = i18n::plural(&self.lang, n);
                m.forms.iter().find(|(c, _)| c == cat)
            }
            _ => m.forms.first(),
        };
        let Some((_, parts)) = form else {
            return String::new();
        };
        let mut out = String::new();
        for p in parts {
            match p {
                MsgPart::Lit(s) => out.push_str(s),
                MsgPart::Hole(h) => match (&m.plural, args.first()) {
                    // a count is digits: it reads the same in any direction
                    (Some(_), Some(a)) => out.push_str(a),
                    // any other value is the program's text, whichever direction it is written in: it is isolated, so that its
                    // punctuation and the sentence around it do not rearrange each other (docs/i18n.md section 8.3)
                    _ => {
                        if let Some(i) = order.iter().position(|o| o == h) {
                            out.push('\u{2068}');
                            out.push_str(args.get(i).map_or("", String::as_str));
                            out.push('\u{2069}');
                        }
                    }
                },
            }
        }
        out
    }
}
