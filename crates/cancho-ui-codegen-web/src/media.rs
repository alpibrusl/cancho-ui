//! `Image` and `Media` (docs/structure.md section 10): the elements are the IR crate's recipe (`cancho_ui_ir::media::image_dom`, `media_dom`);
//! this writes the DOM calls for it, nothing more, so that the prerenderer can write the same elements from the same recipe.

use cancho_ui_ir::ir::IrAccessibility;
use cancho_ui_ir::ir::IrBinding;
use cancho_ui_ir::media::Dom;

use crate::emit::js_string;

pub(crate) fn emit_dom(indent: &str, var: &str, dom: &Dom, parent: &str, out: &mut String) {
    out.push_str(&format!(
        "{indent}const {var} = document.createElement('{}');\n",
        dom.tag
    ));
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
        emit_dom(indent, &format!("{var}_{i}"), c, var, out);
    }
    out.push_str(&format!("{indent}{parent}.appendChild({var});\n"));
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
