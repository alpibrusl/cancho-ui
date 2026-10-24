//! The `Code` block (docs/structure.md section 8): a box around a `<pre><code>`, with a Copy button when `copy: true`. The decisions (is there a
//! button, how tall may it be) are the IR crate's recipe (`cancho_ui_ir::structure::code_spec`); this writes the DOM calls.

use cancho_ui_ir::ir::{IrBinding, IrTextPart, IrViewNode};

use crate::emit::js_string;
use crate::expr::emit_expr_runtime;

pub(crate) fn emit(
    indent: &str,
    var: &str,
    parent_var: &str,
    bindings: &[IrBinding],
    children: &[IrViewNode],
    params: &[&str],
) -> String {
    let (copy, _) = cancho_ui_ir::structure::code_spec(bindings);
    // the box is the recipe the prerenderer writes too (`div.cui-code > pre > code`); its text, and the button, are the script's
    let mut out = String::new();
    crate::media::emit_dom(
        indent,
        var,
        &cancho_ui_ir::recipe::code_skeleton(bindings),
        parent_var,
        &mut out,
    );
    let (pre, code) = (format!("{var}_0"), format!("{var}_0_0"));
    for c in children {
        if let IrViewNode::Text { parts, .. } = c {
            for p in parts {
                match p {
                    IrTextPart::Literal(s) => out.push_str(&format!(
                        "{indent}{code}.appendChild(document.createTextNode({}));\n",
                        js_string(s)
                    )),
                    IrTextPart::Expr(e) => out.push_str(&format!(
                        "{indent}{code}.appendChild(document.createTextNode(String({})));\n",
                        emit_expr_runtime(e, params)
                    )),
                }
            }
        }
    }
    if copy {
        // the only place the page writes to the clipboard: inside the click of this button (a tap), never at load or at a render
        out.push_str(&format!(
            "{indent}const {var}_copy = document.createElement('button');\n{indent}{var}_copy.setAttribute('type', 'button');\n{indent}{var}_copy.classList.add('cui-copy');\n{indent}{var}_copy.setAttribute('aria-label', 'Copy the code');\n{indent}{var}_copy.textContent = 'Copy';\n{indent}{var}_copy.addEventListener('click', function () {{ __cui_copy({var}_copy, {pre}); }});\n{indent}{var}.appendChild({var}_copy);\n"
        ));
    }
    out
}
