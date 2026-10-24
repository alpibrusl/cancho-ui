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
    let (copy, max_height) = cancho_ui_ir::structure::code_spec(bindings);
    let mut out = format!(
        "{indent}const {var} = document.createElement('div');\n{indent}{var}.classList.add('cui-code');\n"
    );
    if copy {
        out.push_str(&format!("{indent}{var}.setAttribute('data-copy', '');\n"));
    }
    out.push_str(&format!(
        "{indent}const {var}_pre = document.createElement('pre');\n{indent}{var}_pre.setAttribute('tabindex', '0');\n"
    ));
    if let Some(h) = max_height {
        out.push_str(&format!("{indent}{var}_pre.style.maxHeight = '{h}rem';\n"));
    }
    out.push_str(&format!(
        "{indent}const {var}_code = document.createElement('code');\n"
    ));
    for c in children {
        if let IrViewNode::Text { parts, .. } = c {
            for p in parts {
                match p {
                    IrTextPart::Literal(s) => out.push_str(&format!(
                        "{indent}{var}_code.appendChild(document.createTextNode({}));\n",
                        js_string(s)
                    )),
                    IrTextPart::Expr(e) => out.push_str(&format!(
                        "{indent}{var}_code.appendChild(document.createTextNode(String({})));\n",
                        emit_expr_runtime(e, params)
                    )),
                }
            }
        }
    }
    out.push_str(&format!(
        "{indent}{var}_pre.appendChild({var}_code);\n{indent}{var}.appendChild({var}_pre);\n"
    ));
    if copy {
        // the only place the page writes to the clipboard: inside the click of this button (a tap), never at load or at a render
        out.push_str(&format!(
            "{indent}const {var}_copy = document.createElement('button');\n{indent}{var}_copy.setAttribute('type', 'button');\n{indent}{var}_copy.classList.add('cui-copy');\n{indent}{var}_copy.setAttribute('aria-label', 'Copy the code');\n{indent}{var}_copy.textContent = 'Copy';\n{indent}{var}_copy.addEventListener('click', function () {{ __cui_copy({var}_copy, {var}_pre); }});\n{indent}{var}.appendChild({var}_copy);\n"
        ));
    }
    out.push_str(&format!("{indent}{parent_var}.appendChild({var});\n"));
    out
}
