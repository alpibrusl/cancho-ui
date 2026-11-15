//! `Key { press: .. on: .. }` (docs/keys.md): a `keydown` listener on the element that holds the key. A `Key` draws nothing.

use cancho_ui_ir::ir::*;
use cancho_ui_ir::keys::{REPEATING, parse_combo};

use crate::emit::STATE_PREFIX;
use crate::emit::js_string;
use crate::expr::emit_expr_runtime;

/// The listener of one key on `parent`. The arguments are evaluated at the render (a constant of it), the combination is read from `e.key` and the two modifiers only, and a key that came from an
/// editable element, or while an IME composes, or with `Alt` or `Meta` held, fires nothing.
pub fn emit(
    indent: &str,
    id: &str,
    parent: &str,
    bindings: &[IrBinding],
    params: &[&str],
) -> String {
    let mut out = String::new();
    let press = bindings.iter().find_map(|b| match (&b.key[..], &b.value) {
        ("press", IrBindingValue::String(s)) => Some(s.as_str()),
        _ => None,
    });
    let Some(combo) = press.and_then(parse_combo) else {
        return out;
    };
    let call = bindings.iter().find_map(|b| match (&b.key[..], &b.value) {
        ("on", IrBindingValue::EventRef(n)) => Some((n.as_str(), &[][..])),
        ("on", IrBindingValue::EventCall { name, args }) => Some((name.as_str(), args.as_slice())),
        _ => None,
    });
    let Some((event, args)) = call else {
        return out;
    };
    let mut names = Vec::with_capacity(args.len());
    for (i, a) in args.iter().enumerate() {
        let arg = format!("{id}_arg{i}");
        out.push_str(&format!(
            "{0}const {1} = {2};\n",
            indent,
            arg,
            emit_expr_runtime(a, params)
        ));
        names.push(arg);
    }
    let disabled = bindings.iter().find_map(|b| match (&b.key[..], &b.value) {
        ("disabled", IrBindingValue::StateRef(n)) => Some(format!("{STATE_PREFIX}{n}")),
        _ => None,
    });
    let repeats = REPEATING.contains(&combo.key.as_str());
    let letter = combo.key.len() == 1;
    out.push_str(&format!(
        "{0}if (!{1}.hasAttribute('tabindex') && !/^(BUTTON|INPUT|TEXTAREA|SELECT|A)$/.test({1}.tagName)) {{ {1}.setAttribute('tabindex', '0'); }}\n",
        indent, parent
    ));
    out.push_str(&format!(
        "{0}{1}.addEventListener('keydown', function (e) {{\n",
        indent, parent
    ));
    if let Some(d) = disabled {
        out.push_str(&format!("{indent}    if ({d}) {{ return; }}\n"));
    }
    out.push_str(&format!(
        "{0}    const t = e.target;\n{0}    if (t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName))) {{ return; }}\n{0}    if (e.isComposing || e.altKey || e.metaKey) {{ return; }}\n",
        indent
    ));
    let key = if letter {
        "e.key.length === 1 ? e.key.toLowerCase() : e.key".to_string()
    } else if combo.key == "Space" {
        "e.key === ' ' ? 'Space' : e.key".to_string()
    } else {
        "e.key".to_string()
    };
    out.push_str(&format!(
        "{0}    if (({1}) !== {2} || e.ctrlKey !== {3} || e.shiftKey !== {4}) {{ return; }}\n",
        indent,
        key,
        js_string(&combo.key),
        combo.control,
        combo.shift
    ));
    // the innermost container that binds the combination takes it: a key of a container inside this one does not also fire this one's
    out.push_str(&format!(
        "{indent}    e.preventDefault();\n{indent}    e.stopPropagation();\n"
    ));
    if !repeats {
        out.push_str(&format!("{indent}    if (e.repeat) {{ return; }}\n"));
    }
    out.push_str(&format!(
        "{0}    __on_{1}({2});\n{0}}});\n",
        indent,
        event,
        names.join(", ")
    ));
    out
}
