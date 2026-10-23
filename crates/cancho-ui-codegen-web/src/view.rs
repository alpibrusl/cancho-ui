//! The view: a component's `view` block as the JavaScript that builds its DOM, with the slots of the component
//! instances it hosts.

use cancho_ui_ir::ir::*;

use crate::emit::{STATE_PREFIX, js_string};
use crate::expr::emit_expr_runtime;

/// Tier 15: does this view subtree contain a component instance
/// (a named element that names a declared component)? Decides
/// whether the enclosing component carries the slot table.
pub(crate) fn view_has_instance(node: &IrViewNode, component_names: &[String]) -> bool {
    match node {
        IrViewNode::Element { kind, children, .. } => {
            if let IrElementKind::Named(name) = kind
                && component_names.iter().any(|d| d == name)
            {
                return true;
            }
            children
                .iter()
                .any(|c| view_has_instance(c, component_names))
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => children
            .iter()
            .any(|c| view_has_instance(c, component_names)),
        IrViewNode::Text { .. } => false,
    }
}

// The view walk threads one piece of state per concern (ids, slots, props);
// a context struct is the follow-up, tracked in ROADMAP.md.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_view_node(
    next_id: &mut u32,
    indent: &str,
    node: &IrViewNode,
    parent_var: &str,
    params: &[&str],
    components: &[String],
    prop_consumers: &[String],
    has_slots: bool,
    slot_prefix: &str,
    // Tier 17: inside a slotting `For` body, the LHS expression
    // (`__arr2[__i2]`) the instance branch assigns each created
    // slot to, so the For's prune loop can find (and dispose) the
    // slots beyond the list length. `None` elsewhere.
    slot_arr: Option<&str>,
) -> String {
    match node {
        IrViewNode::Text { parts, .. } => {
            let id = *next_id;
            *next_id += 1;
            let var = format!("__n{}", id);
            let mut out = format!(
                "{0}const {1} = document.createElement('span');\n",
                indent, var
            );
            for p in parts {
                match p {
                    IrTextPart::Literal(s) => {
                        out.push_str(&format!(
                            "{0}{1}.appendChild(document.createTextNode({2}));\n",
                            indent,
                            var,
                            js_string(s)
                        ));
                    }
                    IrTextPart::Expr(e) => {
                        out.push_str(&format!(
                            "{0}{1}.appendChild(document.createTextNode(String({2})));\n",
                            indent,
                            var,
                            emit_expr_runtime(e, params)
                        ));
                    }
                }
            }
            out.push_str(&format!(
                "{0}{1}.appendChild({2});\n",
                indent, var, parent_var
            ));
            out
        }
        IrViewNode::For {
            over,
            binding,
            children,
            span: _,
        } => {
            // Tier 8 row repetition: a `for..of` over the list,
            // children emitted with `binding` added to the visible
            // name set so it resolves as the loop variable.
            let mut local_params: Vec<&str> = params.to_vec();
            local_params.push(binding.as_str());
            // Tier 15: a loop that contains (below it) a component
            // instance gets slot machinery — one persistent wrapper
            // per (site, index), pruned to the current length. The
            // loop switches to `forEach` to have the index at hand;
            // loops without instances keep the plain `for..of`.
            if has_slots && children.iter().any(|c| view_has_instance(c, components)) {
                let id = *next_id;
                *next_id += 1;
                let list_var = format!("__list{}", id);
                let key_var = format!("__key{}", id);
                let arr_var = format!("__arr{}", id);
                let child_prefix =
                    format!("({0} + \"s{1}\" + __i{2} + \"_\")", slot_prefix, id, id);
                let mut out = format!(
                    "{0}const {1} = {2};\n",
                    indent,
                    list_var,
                    emit_expr_runtime(over, params)
                );
                out.push_str(&format!(
                    "{0}const {1} = {2} + \"s{3}\";\n",
                    indent, key_var, slot_prefix, id
                ));
                out.push_str(&format!(
                    "{0}const {1} = __cui_slots[{2}] || (__cui_slots[{2}] = []);\n",
                    indent, arr_var, key_var
                ));
                // Tier 17: slots dropped by a shrink are *disposed*
                // — their pending awaits resolve Cancelled and the
                // component's own children dispose transitively —
                // and their keys are forgotten, so growing the list
                // later mounts a fresh component instead of
                // resurrecting a disposed one.
                out.push_str(&format!(
                    "{0}while ({1}.length > {2}.length) {{\n",
                    indent, arr_var, list_var
                ));
                out.push_str(&format!(
                    "{0}    const __dead{1} = {2}.pop();\n",
                    indent, id, arr_var
                ));
                out.push_str(&format!(
                    "{0}    delete __cui_slots[__dead{1}.key];\n",
                    indent, id
                ));
                out.push_str(&format!(
                    "{0}    if (__dead{1}.inst) {{ __dead{1}.inst.__cui_dispose(); }}\n",
                    indent, id
                ));
                out.push_str(&format!("{0}}}\n", indent));
                out.push_str(&format!(
                    "{0}{1}.forEach(function ({2}, __i{3}) {{\n",
                    indent, list_var, binding, id
                ));
                let child_indent = format!("{}    ", indent);
                let child_arr = format!("{0}[__i{1}]", arr_var, id);
                for c in children {
                    out.push_str(&emit_view_node(
                        next_id,
                        &child_indent,
                        c,
                        parent_var,
                        &local_params,
                        components,
                        prop_consumers,
                        has_slots,
                        &child_prefix,
                        Some(&child_arr),
                    ));
                }
                out.push_str(&format!("{0}}});\n", indent));
                return out;
            }
            let mut out = format!(
                "{0}for (const {1} of {2}) {{\n",
                indent,
                binding,
                emit_expr_runtime(over, params)
            );
            let child_indent = format!("{}    ", indent);
            for c in children {
                out.push_str(&emit_view_node(
                    next_id,
                    &child_indent,
                    c,
                    parent_var,
                    &local_params,
                    components,
                    prop_consumers,
                    has_slots,
                    slot_prefix,
                    None,
                ));
            }
            out.push_str(&format!("{0}}}\n", indent));
            out
        }
        IrViewNode::If {
            cond,
            children,
            span,
        } => {
            // A component instance under an `If` must be disposed when the condition turns false, as one under a `For`
            // is when the list shrinks: the `If` is emitted as a loop over a list of one element or none, which has
            // that machinery (docs/view-conditional.md section 5).
            if has_slots && children.iter().any(|c| view_has_instance(c, components)) {
                let list = |items: Vec<IrExpr>| IrExpr::ListLit { items, span: *span };
                let synthetic = IrViewNode::For {
                    over: IrExpr::If {
                        cond: Box::new(cond.clone()),
                        then: Box::new(list(vec![IrExpr::IntLit {
                            value: 0,
                            span: *span,
                        }])),
                        else_: Box::new(list(Vec::new())),
                        span: *span,
                    },
                    binding: "__if".to_string(),
                    children: children.clone(),
                    span: *span,
                };
                return emit_view_node(
                    next_id,
                    indent,
                    &synthetic,
                    parent_var,
                    params,
                    components,
                    prop_consumers,
                    has_slots,
                    slot_prefix,
                    slot_arr,
                );
            }
            let mut out = format!("{0}if ({1}) {{\n", indent, emit_expr_runtime(cond, params));
            let child_indent = format!("{}    ", indent);
            for c in children {
                out.push_str(&emit_view_node(
                    next_id,
                    &child_indent,
                    c,
                    parent_var,
                    params,
                    components,
                    prop_consumers,
                    has_slots,
                    slot_prefix,
                    None,
                ));
            }
            out.push_str(&format!("{0}}}\n", indent));
            out
        }
        IrViewNode::Element {
            kind,
            accessibility,
            bindings,
            children,
            span: _,
        } => {
            let id = *next_id;
            *next_id += 1;
            let var = format!("__n{}", id);
            let tag = kind.tag_name();
            let instance_name: Option<&String> = match kind {
                IrElementKind::Named(name) => {
                    if has_slots && components.iter().any(|d| d == name) {
                        Some(name)
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(inst_name) = instance_name {
                // Tier 15: the instance site takes the slot path.
                // One persistent wrapper per site; the DOM subtree
                // (and, with it, the child's closures and state)
                // survives parent re-renders. Nothing else about an
                // instance is emitted — an instance takes no
                // bindings of its own besides its properties (the
                // checker refuses the rest, `instance-opaque`), no
                // children, no event wiring (`instance-opaque`).
                let mut props_static: Vec<(String, &IrExpr)> = Vec::new();
                let mut props_dynamic: Vec<(String, &IrExpr)> = Vec::new();
                for b in bindings {
                    if let IrBindingValue::Const { expr, .. } = &b.value {
                        if matches!(expr.as_ref(), IrExpr::Ident { .. }) {
                            props_dynamic.push((b.key.clone(), expr));
                        } else {
                            props_static.push((b.key.clone(), expr));
                        }
                    }
                }
                let child_takes = prop_consumers.iter().any(|d| d == inst_name);
                let key_var = format!("__sk{0}", id);
                let slot_var = format!("__slot{0}", id);
                let mut out = format!(
                    "{0}const {1} = {2} + \"{3}\";\n",
                    indent,
                    key_var,
                    slot_prefix,
                    format_args!("s{}", id)
                );
                out.push_str(&format!(
                    "{0}let {1} = __cui_slots[{2}];\n",
                    indent, slot_var, key_var
                ));
                out.push_str(&format!("{0}if ({1} === undefined) {{\n", indent, slot_var));
                // Tier 17: the slot remembers its own table key so a
                // For prune can delete the entry (no resurrection of
                // a disposed slot when the list grows again); inside
                // a slotting For body the slot is recorded in the
                // For's per-index array (`__arr2[__i2] = __slot`).
                out.push_str(&format!(
                    "{0}    {1} = __cui_slots[{2}] = {{ el: document.createElement('div'), key: {2} }};\n",
                    indent, slot_var, key_var
                ));
                if let Some(arr_lhs) = slot_arr {
                    out.push_str(&format!("{0}    {1} = {2};\n", indent, arr_lhs, slot_var));
                }
                if child_takes {
                    let mut all: Vec<(String, &IrExpr)> = props_static.clone();
                    all.extend(props_dynamic.clone());
                    let rendered: Vec<String> = all
                        .iter()
                        .map(|(k, v)| format!("{0}: {1}", k, emit_expr_runtime(v, params)))
                        .collect();
                    out.push_str(&format!(
                        "{0}    /*CUI_SETCREDS*/{1}.inst = mount_{2}({1}.el, {{{3}}});\n",
                        indent,
                        slot_var,
                        inst_name,
                        rendered.join(", ")
                    ));
                } else {
                    out.push_str(&format!(
                        "{0}    /*CUI_SETCREDS*/{1}.inst = mount_{2}({1}.el);\n",
                        indent, slot_var, inst_name
                    ));
                }
                out.push_str(&format!("{0}}}\n", indent));
                if child_takes && !props_dynamic.is_empty() {
                    let rendered: Vec<String> = props_dynamic
                        .iter()
                        .map(|(k, v)| format!("{0}: {1}", k, emit_expr_runtime(v, params)))
                        .collect();
                    out.push_str(&format!(
                        "{0}else {{ {1}.inst.__cui_update({{{2}}}); }}\n",
                        indent,
                        slot_var,
                        rendered.join(", ")
                    ));
                }
                out.push_str(&format!(
                    "{0}{1}.appendChild({2}.el);\n",
                    indent, parent_var, slot_var
                ));
                return out;
            }
            let mut out = format!(
                "{0}const {1} = document.createElement('{2}');\n",
                indent, var, tag
            );
            if let Some(cls) = kind.layout_class() {
                out.push_str(&format!("{0}{1}.classList.add('{2}');\n", indent, var, cls));
            }
            if let Some(role) = &accessibility.role {
                let r = match role {
                    IrRole::Text => "text",
                    IrRole::Heading => "heading",
                    IrRole::Button => "button",
                    IrRole::Other(s) => s.as_str(),
                };
                out.push_str(&format!(
                    "{0}{1}.setAttribute('role', {2});\n",
                    indent,
                    var,
                    js_string(r)
                ));
            }
            if let Some(label) = &accessibility.label {
                out.push_str(&format!(
                    "{0}{1}.setAttribute('aria-label', {2});\n",
                    indent,
                    var,
                    js_string(label)
                ));
            }

            // A table's rows go in its `<tbody>` (docs/presentation.md section 4); a header cell says it heads a column.
            let child_parent = if matches!(kind, IrElementKind::Table) {
                let body = format!("{var}_body");
                out.push_str(&format!(
                    "{0}const {1} = document.createElement('tbody');\n{0}{2}.appendChild({1});\n",
                    indent, body, var
                ));
                body
            } else {
                var.clone()
            };
            if matches!(kind, IrElementKind::HeaderCell) {
                out.push_str(&format!(
                    "{0}{1}.setAttribute('scope', 'col');\n",
                    indent, var
                ));
            }

            // A label that is an expression (docs/row-actions.md): evaluated once per render, here, so the accessible
            // name and a button's visible text are the same string.
            let label_var = format!("{var}_label");
            if let Some(e) = &accessibility.label_expr {
                out.push_str(&format!(
                    "{0}const {1} = String({2});\n{0}{3}.setAttribute('aria-label', {1});\n",
                    indent,
                    label_var,
                    emit_expr_runtime(e, params),
                    var
                ));
            }

            // Apply bindings.
            let mut click_handler: Option<String> = None;
            // `on: Pick(m)`: the event and the names of the constants holding its arguments, evaluated at render time.
            let mut click_call: Option<(String, Vec<String>)> = None;
            let mut input_handler: Option<String> = None;
            let mut submit_handler: Option<String> = None;
            let mut input_value: Option<String> = None;
            let mut input_type: Option<String> = None;
            // `error: msg` (docs/controls.md section 7): the state field holding the message of this control.
            let mut field_error: Option<String> = None;
            // `checked: flag` / `on: Toggle` of a `Checkbox` (docs/controls.md section 9).
            let mut checked_field: Option<String> = None;
            let mut change_handler: Option<String> = None;
            // `options:` and `value:` of a `Select` (docs/controls.md section 10): the JavaScript list, the state field.
            let mut options_expr: Option<String> = None;
            let mut select_handler: Option<String> = None;
            let mut button_label: Option<String> = None;
            // Tier 13: on a component instance, `Const` bindings
            // are *properties* — constant initial values for the
            // child's state fields. They go into the mount call's
            // props object, not into wrapper attributes.
            let mut props_obj: Vec<(String, &IrExpr)> = Vec::new();
            for b in bindings {
                let as_prop = match &b.value {
                    IrBindingValue::Const { expr, .. } => Some(expr),
                    _ => None,
                };
                if matches!(kind, IrElementKind::Named(_))
                    && let Some(expr) = as_prop
                {
                    props_obj.push((b.key.clone(), expr));
                    continue;
                }
                match &b.value {
                    IrBindingValue::Const { expr, .. } if b.key == "disabled" => {
                        // `disabled: true` (docs/controls.md): the native attribute, which also stops the click
                        // and Enter on a button and the typing in a field.
                        out.push_str(&format!(
                            "{0}{1}.disabled = {2};\n",
                            indent,
                            var,
                            emit_expr_runtime(expr, params)
                        ));
                    }
                    IrBindingValue::Const { expr, .. } if b.key == "gap" => {
                        // A step of the scale in the default stylesheet (docs/presentation.md section 2).
                        if let IrExpr::IntLit { value, .. } = **expr {
                            out.push_str(&format!(
                                "{0}{1}.classList.add('cui-gap-{2}');\n",
                                indent, var, value
                            ));
                        }
                    }
                    IrBindingValue::Const { expr, .. } if b.key == "max_width" => {
                        if let IrExpr::IntLit { value, .. } = **expr {
                            out.push_str(&format!(
                                "{0}{1}.style.maxWidth = '{2}ch';\n{0}{1}.style.marginInline = 'auto';\n",
                                indent, var, value
                            ));
                        }
                    }
                    IrBindingValue::String(s) if b.key == "tone" => {
                        out.push_str(&format!(
                            "{0}{1}.setAttribute('data-tone', {2});\n",
                            indent,
                            var,
                            js_string(s)
                        ));
                    }
                    IrBindingValue::StateRef(name) if b.key == "error" => {
                        field_error = Some(name.clone());
                    }
                    IrBindingValue::Const { expr, .. } if b.key == "to" => {
                        // A `NavLink` points at a route of this page: the fragment, always, so it cannot name another
                        // origin or a `javascript:` URL (docs/routing.md section 4).
                        out.push_str(&format!(
                            "{0}{1}.setAttribute('href', '#' + String({2}));\n",
                            indent,
                            var,
                            emit_expr_runtime(expr, params)
                        ));
                    }
                    IrBindingValue::Const { expr, .. } if b.key == "options" => {
                        options_expr = Some(emit_expr_runtime(expr, params));
                    }
                    IrBindingValue::StateRef(name) if b.key == "options" => {
                        options_expr = Some(format!("{STATE_PREFIX}{name}"));
                    }
                    IrBindingValue::StateRef(name) if b.key == "checked" => {
                        checked_field = Some(name.clone());
                    }
                    IrBindingValue::StateRef(name) if b.key == "tone" => {
                        out.push_str(&format!(
                            "{0}{1}.setAttribute('data-tone', String({2}{3}));\n",
                            indent, var, STATE_PREFIX, name
                        ));
                    }
                    IrBindingValue::Const { expr, .. }
                        if b.key == "maxlength" || b.key == "rows" =>
                    {
                        // An integer in range (`input-attribute-invalid`): the browser stops typing and pasting at `maxlength`,
                        // and `rows` is the height of a `TextArea`.
                        if let IrExpr::IntLit { value, .. } = **expr {
                            out.push_str(&format!(
                                "{0}{1}.setAttribute('{3}', '{2}');\n",
                                indent, var, value, b.key
                            ));
                        }
                    }
                    IrBindingValue::Const { .. } => {
                        // A `Const` binding on a primitive (a
                        // non-string label etc.) is not expressible
                        // as an HTML attribute value today.
                        out.push_str(&format!(
                            "{0}// property value on a primitive cannot be set as an attribute (Tier 13)\n",
                            indent
                        ));
                    }
                    IrBindingValue::String(s) => {
                        if b.key == "label" {
                            if matches!(kind, IrElementKind::Button) {
                                button_label = Some(s.clone());
                            } else {
                                out.push_str(&format!(
                                    "{0}{1}.setAttribute('aria-label', {2});\n",
                                    indent,
                                    var,
                                    js_string(s)
                                ));
                            }
                        } else if b.key == "type" {
                            // Written once, below, with the other attributes of a TextField (`input-type-invalid`
                            // already kept anything but a known type out).
                            input_type = Some(s.clone());
                        } else if b.key == "value" {
                            if kind.is_field() {
                                input_value = Some(s.clone());
                            } else {
                                out.push_str(&format!(
                                    "{0}// value: on a non-TextField (Phase 2+)\n",
                                    indent
                                ));
                            }
                        } else {
                            out.push_str(&format!(
                                "{0}{1}.setAttribute({2}, {3});\n",
                                indent,
                                var,
                                js_string(&b.key),
                                js_string(s)
                            ));
                        }
                    }
                    IrBindingValue::EventRef(name) => {
                        if b.key == "on" {
                            if matches!(kind, IrElementKind::Button) {
                                click_handler = Some(name.clone());
                            } else if kind.is_field() {
                                input_handler = Some(name.clone());
                            } else if matches!(kind, IrElementKind::Checkbox) {
                                change_handler = Some(name.clone());
                            } else if matches!(kind, IrElementKind::Select) {
                                select_handler = Some(name.clone());
                            } else {
                                out.push_str(&format!(
                                    "{0}// on: {1} on a non-button/non-textfield primitive (Phase 2+)\n",
                                    indent, name
                                ));
                            }
                        } else if b.key == "submit" {
                            // Enter in a TextField (docs/controls.md section 3); a refusal already kept it off
                            // anything else.
                            submit_handler = Some(name.clone());
                        } else {
                            out.push_str(&format!(
                                "{0}// {1}: {2} on a primitive (Phase 2+)\n",
                                indent, b.key, name
                            ));
                        }
                    }
                    IrBindingValue::EventCall { name, args } => {
                        // Only a Button gets here (`event-args-unsupported` keeps it off everything else). Each
                        // argument is a constant of this render, so a row's click names the row it was drawn for.
                        let mut names = Vec::with_capacity(args.len());
                        for (i, a) in args.iter().enumerate() {
                            let arg = format!("{var}_arg{i}");
                            out.push_str(&format!(
                                "{0}const {1} = {2};\n",
                                indent,
                                arg,
                                emit_expr_runtime(a, params)
                            ));
                            names.push(arg);
                        }
                        click_call = Some((name.clone(), names));
                    }
                    IrBindingValue::StateRef(name) if b.key == "disabled" => {
                        // `disabled: sending`: read from the Bool state field on every render.
                        out.push_str(&format!(
                            "{0}{1}.disabled = !!{2}{3};\n",
                            indent, var, STATE_PREFIX, name
                        ));
                    }
                    IrBindingValue::StateRef(name) => {
                        if b.key == "value" {
                            if kind.is_field() || matches!(kind, IrElementKind::Select) {
                                // One-way binding to a state field;
                                // re-render reflects the new value.
                                input_value = Some(format!("{}{}", STATE_PREFIX, name));
                            } else {
                                out.push_str(&format!(
                                    "{0}// value: state ref on a non-TextField (Phase 2+)\n",
                                    indent
                                ));
                            }
                        } else {
                            out.push_str(&format!(
                                "{0}// {1}: {2} on a primitive (Phase 2+)\n",
                                indent, b.key, name
                            ));
                        }
                    }
                    IrBindingValue::Role(r) => {
                        let role_str = match r {
                            IrRole::Text => "text",
                            IrRole::Heading => "heading",
                            IrRole::Button => "button",
                            IrRole::Other(s) => s.as_str(),
                        };
                        out.push_str(&format!(
                            "{0}{1}.setAttribute('role', {2});\n",
                            indent,
                            var,
                            js_string(role_str)
                        ));
                    }
                }
            }

            // Children.
            let child_indent = format!("{}    ", indent);
            if kind.holds_text()
                && children
                    .iter()
                    .all(|c| matches!(c, IrViewNode::Text { .. }))
            {
                for c in children {
                    if let IrViewNode::Text { parts, .. } = c {
                        for p in parts {
                            match p {
                                IrTextPart::Literal(s) => {
                                    out.push_str(&format!(
                                        "{0}{1}.appendChild(document.createTextNode({2}));\n",
                                        indent,
                                        var,
                                        js_string(s)
                                    ));
                                }
                                IrTextPart::Expr(e) => {
                                    out.push_str(&format!(
                                        "{0}{1}.appendChild(document.createTextNode(String({2})));\n",
                                        indent, var, emit_expr_runtime(e, params)
                                    ));
                                }
                            }
                        }
                    }
                }
            } else {
                // A Button's visible text is its `label` when it has no children of its own: the `label` reaches
                // here as the accessibility label (not as a binding), and a button with an `aria-label` and an
                // empty text node showed nothing.
                let visible_label = button_label.clone().or_else(|| {
                    if matches!(kind, IrElementKind::Button | IrElementKind::NavLink)
                        && children.is_empty()
                    {
                        accessibility.label.clone()
                    } else {
                        None
                    }
                });
                if accessibility.label_expr.is_some()
                    && matches!(kind, IrElementKind::Button | IrElementKind::NavLink)
                    && children.is_empty()
                {
                    out.push_str(&format!(
                        "{0}{1}.appendChild(document.createTextNode({2}));\n",
                        indent, var, label_var
                    ));
                } else if let Some(ref label) = visible_label {
                    out.push_str(&format!(
                        "{0}{1}.appendChild(document.createTextNode({2}));\n",
                        indent,
                        var,
                        js_string(label)
                    ));
                } else if matches!(kind, IrElementKind::Button) {
                    out.push_str(&format!(
                        "{0}{1}.appendChild(document.createTextNode(\"\"));\n",
                        indent, var
                    ));
                }
                for c in children {
                    out.push_str(&emit_view_node(
                        next_id,
                        &child_indent,
                        c,
                        &child_parent,
                        params,
                        components,
                        prop_consumers,
                        has_slots,
                        slot_prefix,
                        slot_arr,
                    ));
                }
            }

            // A status text with nothing to say has no tone: the stylesheet would draw its mark (a cross, a check) beside
            // nothing (docs/presentation.md section 3). The element stays, with its role, so a message put there at the next
            // render is still in a live region.
            if bindings.iter().any(|b| b.key == "tone") {
                out.push_str(&format!(
                    "{0}if (!{1}.textContent.trim()) {{ {1}.removeAttribute('data-tone'); }}\n",
                    indent, var
                ));
            }

            // A field: set the type (a TextField), and the initial value.
            if kind.is_field() {
                if matches!(kind, IrElementKind::TextField) {
                    out.push_str(&format!(
                        // One of `INPUT_TYPES` (`input-type-invalid`), plain lowercase words: no escaping to do.
                        "{0}{1}.setAttribute('type', '{2}');\n",
                        indent,
                        var,
                        input_type.as_deref().unwrap_or("text")
                    ));
                }
                if let Some(ref v) = input_value {
                    out.push_str(&format!("{0}{1}.value = {2};\n", indent, var, v));
                }
            }

            // A select: one `<option>` per string of the list, then the chosen one. A value that is not in the list selects
            // nothing (the box is blank) rather than the first option, so the screen never says what the state does not.
            if matches!(kind, IrElementKind::Select) {
                if let Some(list) = &options_expr {
                    out.push_str(&format!(
                        "{0}for (const __o of {1}) {{ const __opt = document.createElement('option'); __opt.value = String(__o); __opt.textContent = String(__o); {2}.appendChild(__opt); }}\n",
                        indent, list, var
                    ));
                }
                if let Some(v) = &input_value {
                    out.push_str(&format!("{0}{1}.value = {2};\n", indent, var, v));
                }
            }

            // A checkbox is an `<input type="checkbox">` drawn from its `Bool`.
            if matches!(kind, IrElementKind::Checkbox) {
                out.push_str(&format!(
                    "{0}{1}.setAttribute('type', 'checkbox');\n",
                    indent, var
                ));
                if let Some(name) = &checked_field {
                    out.push_str(&format!(
                        "{0}{1}.checked = !!{2}{3};\n",
                        indent, var, STATE_PREFIX, name
                    ));
                }
            }

            // Append to parent. A field with a label is drawn inside a `<label>` that shows it
            // (docs/presentation.md section 1); the field keeps its `aria-label`.
            let field_label = if kind.is_field() || matches!(kind, IrElementKind::Select) {
                accessibility
                    .label
                    .as_ref()
                    .map(|l| js_string(l))
                    .or_else(|| accessibility.label_expr.as_ref().map(|_| label_var.clone()))
            } else {
                None
            };
            let box_label = if matches!(kind, IrElementKind::Checkbox) {
                accessibility
                    .label
                    .as_ref()
                    .map(|l| js_string(l))
                    .or_else(|| accessibility.label_expr.as_ref().map(|_| label_var.clone()))
            } else {
                None
            };
            if let Some(text) = box_label {
                // The box first, then the text, in one label: clicking the text toggles the box.
                out.push_str(&format!(
                    "{0}const {1}_field = document.createElement('label');\n{0}{1}_field.classList.add('cui-check');\n{0}{1}_field.appendChild({1});\n{0}{1}_field.appendChild(document.createTextNode({2}));\n{0}{3}.appendChild({1}_field);\n",
                    indent, var, text, parent_var
                ));
            } else if let Some(text) = field_label {
                out.push_str(&format!(
                    "{0}const {1}_field = document.createElement('label');\n{0}{1}_field.classList.add('cui-field');\n{0}{1}_field.appendChild(document.createTextNode({2}));\n{0}{1}_field.appendChild({1});\n{0}{3}.appendChild({1}_field);\n",
                    indent, var, text, parent_var
                ));
            } else {
                out.push_str(&format!(
                    "{0}{2}.appendChild({1});\n",
                    indent, var, parent_var
                ));
            }

            // The message that goes with the control: a status element next to it, with an id the control points at while it
            // holds text (docs/controls.md section 7). The control is marked invalid only then. The id is unique across the page
            // even in a `For`, so it comes from a counter, not from the node number.
            if let Some(name) = &field_error {
                out.push_str(&format!(
                    "{0}const {1}_err = document.createElement('span');\n\
{0}{1}_err.id = 'cui-err-' + (window.__cui_uid = (window.__cui_uid || 0) + 1);\n\
{0}{1}_err.setAttribute('role', 'status');\n\
{0}{1}_err.setAttribute('data-tone', 'error');\n\
{0}{1}_err.appendChild(document.createTextNode(String({4}{2})));\n\
{0}if ({1}_err.textContent.trim()) {{ {1}.setAttribute('aria-invalid', 'true'); {1}.setAttribute('aria-describedby', {1}_err.id); }} else {{ {1}_err.removeAttribute('data-tone'); }}\n\
{0}{3}.appendChild({1}_err);\n",
                    indent, var, name, parent_var, STATE_PREFIX
                ));
            }

            // Wire up event handlers.
            if let Some(event_name) = click_handler {
                out.push_str(&format!(
                    "{0}{1}.addEventListener('click', __on_{2});\n",
                    indent, var, event_name
                ));
            }
            if let Some((event_name, arg_names)) = click_call {
                out.push_str(&format!(
                    "{0}{1}.addEventListener('click', function () {{ __on_{2}({3}); }});\n",
                    indent,
                    var,
                    event_name,
                    arg_names.join(", ")
                ));
            }
            if let Some(event_name) = submit_handler {
                out.push_str(&format!(
                    "{0}{1}.addEventListener('keydown', function (e) {{ if (e.key === 'Enter' && !e.isComposing && !e.repeat) {{ e.preventDefault(); if (!(__submit_blocked.{2} && __submit_blocked.{2}())) {{ __on_{2}(); }} }} }});\n",
                    indent, var, event_name
                ));
            }
            if let Some(event_name) = select_handler {
                out.push_str(&format!(
                    "{0}{1}.addEventListener('change', function (e) {{ __on_{2}(e.target.value); }});\n",
                    indent, var, event_name
                ));
            }
            if let Some(event_name) = change_handler {
                out.push_str(&format!(
                    "{0}{1}.addEventListener('change', function (e) {{ __on_{2}(e.target.checked); }});\n",
                    indent, var, event_name
                ));
            }
            if let Some(event_name) = input_handler {
                out.push_str(&format!(
                    "{0}{1}.addEventListener('input', function (e) {{ __on_{2}(e.target.value); }});\n",
                    indent, var, event_name
                ));
            }

            out
        }
    }
}
