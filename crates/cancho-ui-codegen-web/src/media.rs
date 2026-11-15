//! `Image` and `Media` (docs/structure.md section 10): the elements are the IR crate's recipe (`cancho_ui_ir::media::image_dom`, `media_dom`);
//! this writes the DOM calls for it, nothing more, so that the prerenderer can write the same elements from the same recipe.

use cancho_ui_ir::ir::IrAccessibility;
use cancho_ui_ir::ir::IrBinding;
use cancho_ui_ir::media::Dom;

use crate::emit::js_string;

pub(crate) fn emit_dom(indent: &str, var: &str, dom: &Dom, parent: &str, out: &mut String) {
    // the elements of a drawing (a Waffle) are in the SVG namespace
    if matches!(
        dom.tag,
        "svg" | "path" | "circle" | "rect" | "line" | "polyline"
    ) {
        out.push_str(&format!(
            "{indent}const {var} = document.createElementNS('http://www.w3.org/2000/svg', '{}');\n",
            dom.tag
        ));
    } else {
        out.push_str(&format!(
            "{indent}const {var} = document.createElement('{}');\n",
            dom.tag
        ));
    }
    for (k, v) in &dom.attrs {
        out.push_str(&format!(
            "{indent}{var}.setAttribute('{k}', {});\n",
            js_string(v)
        ));
    }
    if let Some(t) = &dom.text {
        out.push_str(&format!(
            "{indent}{var}.appendChild(document.createTextNode({}));\n",
            js_string(t)
        ));
    }
    for (i, c) in dom.children.iter().enumerate() {
        // the place of a script-drawn part (a copy button) is not an element: the script's own box draws the part itself
        if c.tag == cancho_ui_ir::recipe::COPY_BUTTON_HOLE {
            continue;
        }
        emit_dom(indent, &format!("{var}_{i}"), c, var, out);
    }
    out.push_str(&format!("{indent}{parent}.appendChild({var});\n"));
}

/// An icon is one call, `__cui_icon(parent, name, class, px, label | null)`, and not the dozen lines of DOM calls that `emit_dom` writes for it:
/// a console of a few hundred icons was 1 MB of script for them. The shapes are in one table (`icon_helper`), once per name; the attributes
/// and their order are the recipe's (`cancho_ui_ir::icon::svg`), read back from it here, so the call cannot draw anything the recipe does not.
pub(crate) fn emit_icon(indent: &str, dom: &Dom, parent: &str, out: &mut String) {
    let attr = |k: &str| {
        dom.attrs
            .iter()
            .find(|(n, _)| *n == k)
            .map(|(_, v)| v.as_str())
            .unwrap_or("")
    };
    let label = if dom.attrs.iter().any(|(n, _)| *n == "aria-label") {
        js_string(attr("aria-label"))
    } else {
        "null".to_string()
    };
    out.push_str(&format!(
        "{indent}__cui_icon({parent}, {}, {}, {}, {label});\n",
        js_string(attr("data-icon")),
        js_string(attr("class")),
        js_string(attr("width")),
    ));
}

/// The table of the shapes of the icons the emitted program calls, and the function that draws one. Empty when it calls none.
pub(crate) fn icon_helper(emitted: &str) -> String {
    let mut names: Vec<&str> = Vec::new();
    let mut rest = emitted;
    while let Some(i) = rest.find("__cui_icon(") {
        rest = &rest[i + "__cui_icon(".len()..];
        let Some(q) = rest.find(", \"") else { break };
        let after = &rest[q + 3..];
        let Some(e) = after.find('"') else { break };
        let name = &after[..e];
        if !names.contains(&name) {
            names.push(name);
        }
    }
    if names.is_empty() {
        return String::new();
    }
    names.sort();
    // the attributes every icon has, in the recipe's order: a probe with markers where a value varies
    let probe = |label: Option<&str>| cancho_ui_ir::icon::svg("\u{1}n", "\u{1}c", "\u{1}p", label);
    let mut fixed = String::new();
    let mut put = |k: &str, v: &str| {
        let v = match v {
            "\u{1}n" => "name".to_string(),
            "\u{1}c" => "cls".to_string(),
            "\u{1}p" => "px".to_string(),
            "\u{1}l" => "label".to_string(),
            other => js_string(other),
        };
        fixed.push_str(&format!("'{k}', {v}, "));
    };
    let with = probe(Some("\u{1}l"));
    let without = probe(None);
    let common = with.attrs.iter().take_while(|(k, _)| *k != "role").count();
    for (k, v) in &with.attrs[..common] {
        put(k, v);
    }
    let mut lab = String::new();
    for (k, v) in &with.attrs[common..] {
        lab.push_str(&format!(
            "'{k}', {}, ",
            if v == "\u{1}l" {
                "label".to_string()
            } else {
                js_string(v)
            }
        ));
    }
    let mut hid = String::new();
    for (k, v) in &without.attrs[common..] {
        hid.push_str(&format!("'{k}', {}, ", js_string(v)));
    }
    let mut table = String::new();
    for name in names {
        let shapes = cancho_ui_ir::icon::shapes(name).unwrap_or_default();
        let parts: Vec<String> = shapes
            .iter()
            .map(|d| {
                let mut a = vec![js_string(d.tag)];
                for (k, v) in &d.attrs {
                    a.push(js_string(k));
                    a.push(js_string(v));
                }
                format!("[{}]", a.join(","))
            })
            .collect();
        table.push_str(&format!("{}:[{}],\n", js_string(name), parts.join(",")));
    }
    format!(
        "// Icons (one call each, `__cui_icon`): the shapes are written once per name. The attributes are the recipe's (cancho_ui_ir::icon::svg).\nconst __cui_icon_shapes = {{\n{table}}};\nfunction __cui_icon(parent, name, cls, px, label) {{\n    const ns = 'http://www.w3.org/2000/svg';\n    const s = document.createElementNS(ns, 'svg');\n    const a = [{fixed}].concat(label === null ? [{hid}] : [{lab}]);\n    for (let i = 0; i < a.length; i += 2) {{ s.setAttribute(a[i], a[i + 1]); }}\n    const sh = __cui_icon_shapes[name] || [];\n    for (let j = 0; j < sh.length; j++) {{ const e = document.createElementNS(ns, sh[j][0]); for (let i = 1; i < sh[j].length; i += 2) {{ e.setAttribute(sh[j][i], sh[j][i + 1]); }} s.appendChild(e); }}\n    parent.appendChild(s);\n}}\n"
    )
}

pub(crate) fn emit_image(
    indent: &str,
    var: &str,
    parent_var: &str,
    bindings: &[IrBinding],
) -> String {
    let mut out = String::new();
    emit_dom(
        indent,
        var,
        &cancho_ui_ir::media::image_dom(bindings),
        parent_var,
        &mut out,
    );
    out
}

pub(crate) fn emit_media(
    indent: &str,
    var: &str,
    parent_var: &str,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
) -> String {
    let mut out = String::new();
    emit_dom(
        indent,
        var,
        &cancho_ui_ir::media::media_dom(accessibility, bindings),
        parent_var,
        &mut out,
    );
    out
}
