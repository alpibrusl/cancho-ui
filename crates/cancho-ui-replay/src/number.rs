//! What a number field gives its event (docs/number-input.md section 3): the text of the number in the field's language as the JSON number
//! text `num(text, default)` reads, or the typed text itself when it is not a number. The emitted `__cui_number_text` is the same function;
//! `tests/number_corpus.json` is checked against both.

use cancho_ui_ir::i18n::Lang;

use crate::value::js_number;

const MARKS: [char; 3] = ['\u{200e}', '\u{200f}', '\u{061c}'];
const SPACE: [char; 6] = [' ', '\t', '\n', '\r', '\u{00a0}', '\u{202f}'];

fn without_marks(s: &str) -> String {
    s.chars().filter(|c| !MARKS.contains(c)).collect()
}

/// A digit of the scripts the field reads: ASCII, Arabic-Indic, extended Arabic-Indic (Persian, Urdu) and full-width.
fn digit(c: char) -> Option<char> {
    let n = c as u32;
    let d = match n {
        0x30..=0x39 => n - 0x30,
        0x0660..=0x0669 => n - 0x0660,
        0x06f0..=0x06f9 => n - 0x06f0,
        0xff10..=0xff19 => n - 0xff10,
        _ => return None,
    };
    char::from_digit(d, 10)
}

/// The number `text` is in the language `lang` (`None`: English), as the text a program can read with `num`: `None` when it is not one.
/// Optional sign (the language's minus, `-`, `U+2212` or `+`), digits, at most one decimal separator (the language's, and `.` when `.` is
/// not its grouping mark) and an exponent; no grouping mark. Spaces around it are ignored. Zero, or a magnitude from 1e-6 up to 1e21 (where
/// JavaScript and replay print a number the same way); anything else is not a number to this function.
pub fn parse_number(text: &str, lang: Option<&Lang>) -> Option<String> {
    let (dec, grp, neg) = match lang {
        Some(l) => (l.decimal, l.group, without_marks(l.minus)),
        None => (".", ",", "-".to_string()),
    };
    let t = without_marks(text);
    let t = t.trim_matches(|c| SPACE.contains(&c));
    let mut rest: Vec<char> = t.chars().map(|c| digit(c).unwrap_or(c)).collect();
    let neg_chars: Vec<char> = neg.chars().collect();
    let mut sign = "";
    if !neg_chars.is_empty() && rest.starts_with(&neg_chars) {
        sign = "-";
        rest.drain(..neg_chars.len());
    } else if matches!(rest.first(), Some('-' | '\u{2212}')) {
        sign = "-";
        rest.remove(0);
    } else if rest.first() == Some(&'+') {
        rest.remove(0);
    }
    let dec_chars: Vec<char> = dec.chars().collect();
    let dot_ok = dec != "." && grp != ".";
    let (mut int, mut frac, mut exp) = (String::new(), String::new(), String::new());
    let mut i = 0;
    while i < rest.len() && rest[i].is_ascii_digit() {
        int.push(rest[i]);
        i += 1;
    }
    let mut has_point = false;
    if rest[i..].starts_with(&dec_chars) && !dec_chars.is_empty() {
        has_point = true;
        i += dec_chars.len();
    } else if dot_ok && rest.get(i) == Some(&'.') {
        has_point = true;
        i += 1;
    }
    if has_point {
        while i < rest.len() && rest[i].is_ascii_digit() {
            frac.push(rest[i]);
            i += 1;
        }
    }
    if int.is_empty() && frac.is_empty() {
        return None;
    }
    if matches!(rest.get(i), Some('e' | 'E')) {
        i += 1;
        if matches!(rest.get(i), Some('+' | '-')) {
            exp.push(rest[i]);
            i += 1;
        }
        let from = i;
        while i < rest.len() && rest[i].is_ascii_digit() {
            exp.push(rest[i]);
            i += 1;
        }
        if i == from {
            return None;
        }
    }
    if i != rest.len() {
        return None;
    }
    let mut s = format!("{sign}{}", if int.is_empty() { "0" } else { &int });
    if !frac.is_empty() {
        s.push('.');
        s.push_str(&frac);
    }
    if !exp.is_empty() {
        s.push('e');
        s.push_str(&exp);
    }
    let n: f64 = s.parse().ok()?;
    if !n.is_finite() || (n != 0.0 && !(1e-6..1e21).contains(&n.abs())) {
        return None;
    }
    js_number(n)
}

/// What the field's event is given for `typed`: the number it is, or the text itself.
pub fn delivered(typed: &str, lang: Option<&Lang>) -> String {
    parse_number(typed, lang).unwrap_or_else(|| typed.to_string())
}

impl crate::machine::Machine<'_> {
    /// The language the number fields of this component read: the language in use when the component has the `Locale` capability, English
    /// otherwise (the page has no language to read when it has no `Locale`).
    pub(crate) fn number_lang(&self) -> Option<&'static Lang> {
        self.comp
            .reachable_effects
            .iter()
            .any(|e| e.capability == "Locale")
            .then(|| self.locale.data())
    }

    /// Is every control in the page that fires `event` a number field (a `TextField` with `type: "number"`)? Then the value of the event is what
    /// `delivered` makes of what was typed. `false` when another control could give it any text, or none fires it.
    pub fn is_number_event(&self, event: &str) -> bool {
        use cancho_ui_ir::ir::{IrBindingValue, IrElementKind, IrViewNode};
        #[derive(Default)]
        struct Seen {
            number: bool,
            other: bool,
        }
        fn walk(m: &crate::machine::Machine<'_>, node: &IrViewNode, event: &str, f: &mut Seen) {
            match node {
                IrViewNode::Element {
                    kind,
                    bindings,
                    children,
                    ..
                } => {
                    let fires = bindings.iter().any(|b| {
                        b.key == "on"
                            && match &b.value {
                                IrBindingValue::EventRef(e) => e == event,
                                IrBindingValue::EventCall { name, .. } => name == event,
                                _ => false,
                            }
                    });
                    if fires {
                        let number = matches!(kind, IrElementKind::TextField)
                            && bindings.iter().any(|b| {
                                b.key == "type"
                                    && matches!(&b.value, IrBindingValue::String(s) if s == "number")
                            });
                        if number {
                            f.number = true;
                        } else {
                            f.other = true;
                        }
                    }
                    for c in children {
                        walk(m, c, event, f);
                    }
                }
                IrViewNode::For { children, .. } => {
                    for c in children {
                        walk(m, c, event, f);
                    }
                }
                IrViewNode::If { cond, children, .. } => {
                    if m.condition_holds(cond) {
                        for c in children {
                            walk(m, c, event, f);
                        }
                    }
                }
                IrViewNode::Text { .. } => {}
            }
        }
        let mut f = Seen::default();
        walk(self, &self.comp.view.body, event, &mut f);
        f.number && !f.other
    }
}
