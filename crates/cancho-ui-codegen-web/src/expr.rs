//! Expressions: an IR expression as a JavaScript expression, in a state initializer (`emit_expr_init`) and at run
//! time (`emit_expr_runtime`, with `match` as nested ternaries and the builtin calls).

use cancho_ui_ir::ir::*;

use crate::emit::{STATE_PREFIX, js_string};

pub(crate) fn emit_expr_init(e: &IrExpr) -> String {
    match e {
        IrExpr::IntLit { value, .. } => value.to_string(),
        IrExpr::FloatLit { value, .. } => format!("{}", value),
        IrExpr::StringLit { value, .. } => js_string(value),
        IrExpr::BoolLit { value, .. } => value.to_string(),
        IrExpr::Ident { .. } => "undefined".into(),
        IrExpr::Call { .. } => "undefined".into(),
        IrExpr::Unary {
            op: IrUnOp::Neg,
            operand,
            ..
        } => match operand.as_ref() {
            IrExpr::IntLit { value, .. } => (-value).to_string(),
            IrExpr::FloatLit { value, .. } => format!("{}", -value),
            _ => "(0 - (".to_string() + &emit_expr_init(operand) + "))",
        },
        IrExpr::Binary { op, lhs, rhs, .. } => {
            "(".to_string()
                + &emit_expr_init(lhs)
                + " "
                + op.js_op()
                + " "
                + &emit_expr_init(rhs)
                + ")"
        }
        // State-field initial values can be `Ok(value)` or
        // `Err(reason)` constructors — Tier 6 lifted the
        // restriction that initial values must be literals. The
        // runtime representation is a tagged object:
        //   {__cui_tag:"Ok", value:v} or {__cui_tag:"Err", reason:r}.
        IrExpr::Ok { value, .. } => {
            "({__cui_tag:\"Ok\",value:".to_string() + &emit_expr_init(value) + "})"
        }
        IrExpr::Err { reason, .. } => {
            "({__cui_tag:\"Err\",reason:".to_string() + &emit_expr_init(reason) + "})"
        }
        // `Some(value)` and `None` are legal state-field
        // initial values (Tier 8): tagged objects like the
        // Result constructors.
        IrExpr::Some { value, .. } => {
            "({__cui_tag:\"Some\",value:".to_string() + &emit_expr_init(value) + "})"
        }
        IrExpr::None { .. } => "({__cui_tag:\"None\"})".to_string(),
        // A `List[T]` literal is a plain JS array; the items are
        // literals or constructor expressions.
        IrExpr::ListLit { items, .. } => {
            let rendered: Vec<String> = items.iter().map(emit_expr_init).collect();
            format!("[{}]", rendered.join(", "))
        }
        // `filter`/`map`/`len` in a state initializer: emitted like
        // the runtime form (Tier 11). A filter/map over another
        // state field in an initializer reads that field before it
        // is initialised — the same forward-reference risk any
        // initializer carries — so prefer literals here.
        IrExpr::Filter {
            list, item, cond, ..
        } => {
            format!(
                "({}).filter(function ({}) {{ return {}; }})",
                emit_expr_init(list),
                item,
                emit_expr_init(cond)
            )
        }
        IrExpr::Map {
            list, item, body, ..
        } => {
            format!(
                "({}).map(function ({}) {{ return {}; }})",
                emit_expr_init(list),
                item,
                emit_expr_init(body)
            )
        }
        IrExpr::SortBy {
            list, item, key, ..
        } => format!(
            "__cui_sort({}, function ({}) {{ return {}; }})",
            emit_expr_init(list),
            item,
            emit_expr_init(key)
        ),
        IrExpr::Len { list, .. } => format!("({}).length", emit_expr_init(list)),
        // `match` is not a legal state-field initial value in Tier 6
        // (a state field's initial is an expression but a match
        // expression is a runtime construct, not a value). Phase 2
        // can lift this if a use case arrives.
        IrExpr::Match { .. } => "undefined".into(),
        // `await(...)` is a runtime construct, not a value — same
        // reasoning as `match`. Phase 2 may lift this if a use case
        // arrives (e.g. a lazy-initialised state).
        IrExpr::Await { .. } | IrExpr::Cancelled { .. } => "undefined".into(),
    }
}

pub(crate) fn emit_expr_runtime(e: &IrExpr, params: &[&str]) -> String {
    match e {
        IrExpr::IntLit { value, .. } => value.to_string(),
        IrExpr::FloatLit { value, .. } => format!("{}", value),
        IrExpr::StringLit { value, .. } => js_string(value),
        IrExpr::BoolLit { value, .. } => value.to_string(),
        IrExpr::Ident { name, .. } => {
            // An identifier that matches a parameter name is a
            // function parameter, not a state field; emit it
            // bare. Everything else gets the state prefix.
            if params.contains(&name.as_str()) {
                name.clone()
            } else {
                STATE_PREFIX.to_string() + name
            }
        }
        IrExpr::Call { path, args, .. } => emit_call_runtime(path, args, params),
        IrExpr::Unary {
            op: IrUnOp::Neg,
            operand,
            ..
        } => "(0 - (".to_string() + &emit_expr_runtime(operand, params) + "))",
        IrExpr::Binary { op, lhs, rhs, .. } => {
            "(".to_string()
                + &emit_expr_runtime(lhs, params)
                + " "
                + op.js_op()
                + " "
                + &emit_expr_runtime(rhs, params)
                + ")"
        }
        IrExpr::Ok { value, .. } => {
            "({__cui_tag:\"Ok\",value:".to_string() + &emit_expr_runtime(value, params) + "})"
        }
        IrExpr::Err { reason, .. } => {
            "({__cui_tag:\"Err\",reason:".to_string() + &emit_expr_runtime(reason, params) + "})"
        }
        IrExpr::Await { request, .. } => {
            // `await(expr)` becomes a cancellation tracked call:
            //  - the request's effect kind is the cancellation
            //    group (`net.search` -> "net_search");
            //  - starting a call cancels the group's still-pending
            //    one (stale-result protection);
            //  - if a cancelled request resolves, the chain throws
            //    the `__cui_cancelled` sentinel, which the handler
            //    body's try/catch swallows — the rest of the
            //    handler is dropped, so a superseded continuation
            //    cannot write state after a newer call completed.
            //  (The enclosing function is generated as `async
            //  function`; see emit_component.)
            let group = match request.as_ref() {
                IrExpr::Call { path, .. } if !path.is_empty() => path.join("_"),
                _ => "default".to_string(),
            };
            // Tier 17: the third argument is the *component
            // instance's* inflight table — two components awaiting
            // the same effect keep independent cancellation groups,
            // and the instance's `__cui_dispose` cancels its own
            // table on disposal.
            format!(
                "await __cui_call(\"{}\", function () {{ return {}; }}, __cui_inflight)",
                group,
                emit_expr_runtime(request, params)
            )
        }
        IrExpr::Cancelled { .. } => "({__cui_tag:\"Cancelled\"})".to_string(),
        IrExpr::Some { value, .. } => {
            "({__cui_tag:\"Some\",value:".to_string() + &emit_expr_runtime(value, params) + "})"
        }
        IrExpr::None { .. } => "({__cui_tag:\"None\"})".to_string(),
        IrExpr::ListLit { items, .. } => {
            let rendered: Vec<String> =
                items.iter().map(|i| emit_expr_runtime(i, params)).collect();
            format!("[{}]", rendered.join(", "))
        }
        IrExpr::Filter {
            list, item, cond, ..
        } => {
            // `filter(list, x => cond)` — an inline
            // `Array.prototype.filter`. The user's `x` becomes the
            // callback parameter and shadows a state field.
            let mut local_params: Vec<&str> = params.to_vec();
            local_params.push(item.as_str());
            format!(
                "({}).filter(function ({}) {{ return {}; }})",
                emit_expr_runtime(list, params),
                item,
                emit_expr_runtime(cond, &local_params)
            )
        }
        IrExpr::Map {
            list, item, body, ..
        } => {
            let mut local_params: Vec<&str> = params.to_vec();
            local_params.push(item.as_str());
            format!(
                "({}).map(function ({}) {{ return {}; }})",
                emit_expr_runtime(list, params),
                item,
                emit_expr_runtime(body, &local_params)
            )
        }
        IrExpr::SortBy {
            list, item, key, ..
        } => {
            let mut local_params: Vec<&str> = params.to_vec();
            local_params.push(item.as_str());
            format!(
                "__cui_sort({}, function ({}) {{ return {}; }})",
                emit_expr_runtime(list, params),
                item,
                emit_expr_runtime(key, &local_params)
            )
        }
        IrExpr::Len { list, .. } => {
            // Element count of a list, character count of a
            // string — the same JS `.length`.
            format!("({}).length", emit_expr_runtime(list, params))
        }
        IrExpr::Match {
            scrutinee, arms, ..
        } => {
            // Lower to nested ternaries. For Tier 6 the scrutinee
            // is expected to be a `Result` (Ok/Err) or `Option`
            // (Some/None). Phase 2 may emit a switch table or a
            // decision tree; ternaries are correct and the
            // smallest thing that works.
            emit_match_as_ternary(scrutinee.as_ref(), arms, params)
        }
    }
}

/// Lower a `match` expression to a single ternary chain. The
/// scrutinee is evaluated once into a `__cui_match_<i>` local so
/// each arm's guard and body can refer to it as both a tag
/// (`__cui_match_<i>.__cui_tag`) and a payload
/// (`__cui_match_<i>.value`).
fn emit_match_as_ternary(scrutinee: &IrExpr, arms: &[IrMatchArm], params: &[&str]) -> String {
    use std::cell::Cell;
    thread_local! {
        static COUNTER: Cell<u32> = const { Cell::new(0) };
    }
    let id = COUNTER.with(|c| {
        let n = c.get() + 1;
        c.set(n);
        n
    });
    let scrutinee_var = format!("__cui_match_{}", id);
    let scrutinee_js = emit_expr_runtime(scrutinee, params);
    let mut out = format!(
        "((function () {{ var {0} = {1}; ",
        scrutinee_var, scrutinee_js
    );

    for (i, arm) in arms.iter().enumerate() {
        let guard = pattern_guard(&arm.pattern, &scrutinee_var);
        let body = emit_arm_body(&arm.pattern, &scrutinee_var, &arm.body, params);
        let sep = if i == 0 { "return " } else { "      " };
        out.push_str(sep);
        out.push_str(&guard);
        out.push_str(" ? ");
        out.push_str(&body);
        if i + 1 < arms.len() {
            out.push_str(" : ");
        } else {
            // Last arm — if no arm matches, this should be
            // unreachable (the type checker refused an
            // incomplete match). Emit a value that throws so
            // bugs surface immediately rather than silently
            // returning undefined.
            out.push_str(" : (function () { throw new Error(\"no match arm matched (non-exhaustive match in source)\"); })()");
        }
    }
    out.push_str("; })())");
    out
}

fn pattern_guard(p: &IrPattern, scrutinee_var: &str) -> String {
    match p {
        IrPattern::Wildcard { .. } => "true".to_string(),
        IrPattern::Ident { .. } => "true".to_string(),
        IrPattern::Variant { name, .. } => {
            format!("({0}.__cui_tag === \"{1}\")", scrutinee_var, name)
        }
        IrPattern::UnitVariant { name, .. } => {
            // Zero-payload variant (e.g. `Cancelled`). Matches if
            // the runtime tag equals the variant name. The variant
            // carries no value, so the arm body cannot bind a
            // payload — `Cancelled =>` is a no-arg pattern.
            format!("({0}.__cui_tag === \"{1}\")", scrutinee_var, name)
        }
    }
}

fn emit_arm_body(p: &IrPattern, scrutinee_var: &str, body: &IrExpr, params: &[&str]) -> String {
    match p {
        IrPattern::Wildcard { .. } | IrPattern::Ident { .. } => emit_expr_runtime(body, params),
        IrPattern::Variant { name, binding, .. } => {
            // The payload field depends on the variant: `Ok`
            // stores its under `value`, `Err` under `reason` (see
            // the tagged constructor emitters).
            // A `_` binding skips the IIFE bind entirely.
            let field = if name == "Err" { "reason" } else { "value" };
            if binding == "_" {
                emit_expr_runtime(body, params)
            } else {
                let mut local_params: Vec<&str> = params.to_vec();
                local_params.push(binding.as_str());
                let body_with_param = emit_expr_runtime(body, &local_params);
                format!(
                    "(function () {{ var {0} = {1}.{2}; return {3}; }})()",
                    binding, scrutinee_var, field, body_with_param
                )
            }
        }
        IrPattern::UnitVariant { .. } => {
            // No payload to bind. Just emit the body.
            emit_expr_runtime(body, params)
        }
    }
}

/// Lower a builtin call to its JavaScript form. The capability
/// that authorises the call has already been verified by the
/// checker; this function only picks the symbol to call.
fn emit_call_runtime(path: &[String], args: &[IrExpr], params: &[&str]) -> String {
    if path.len() == 2 && path[0] == "clock" && path[1] == "now" {
        return "__cui_clock_now()".to_string();
    }
    if path.len() == 1 && path[0] == "sleep" {
        // Tier 7: `sleep(ms)` resolves to
        // `{__cui_tag:"Ok",value:undefined}` after `ms` ms.
        let rendered_args: Vec<String> =
            args.iter().map(|a| emit_expr_runtime(a, params)).collect();
        return format!("__cui_sleep_ms({})", rendered_args.join(", "));
    }
    if path.len() == 2 && path[0] == "storage" && (path[1] == "get" || path[1] == "set") {
        // docs/adapters.md: synchronous; `get(key, default)` answers a String, `set(key, value)` a Bool.
        let rendered_args: Vec<String> =
            args.iter().map(|a| emit_expr_runtime(a, params)).collect();
        return format!(
            "__cui_storage_{}({}, __cui_storage_ns)",
            path[1],
            rendered_args.join(", ")
        );
    }
    if path.len() == 2
        && path[0] == "json"
        && (path[1] == "get" || path[1] == "items" || path[1] == "quote")
    {
        let rendered_args: Vec<String> =
            args.iter().map(|a| emit_expr_runtime(a, params)).collect();
        return format!("__cui_json_{}({})", path[1], rendered_args.join(", "));
    }
    if path.len() == 1
        && matches!(
            path[0].as_str(),
            "reverse"
                | "contains"
                | "lower"
                | "num"
                | "take"
                | "skip"
                | "min"
                | "max"
                | "abs"
                | "floor"
                | "ceil"
                | "round"
                | "lines"
                | "split"
                | "join"
                | "trim"
                | "upper"
                | "starts_with"
                | "ends_with"
        )
    {
        let rendered_args: Vec<String> =
            args.iter().map(|a| emit_expr_runtime(a, params)).collect();
        return format!("__cui_{}({})", path[0], rendered_args.join(", "));
    }
    if path.len() == 2 && path[0] == "location" && path[1] == "current" {
        return "__cui_location_current(__cui_location_digits)".to_string();
    }
    if path.len() == 2 && path[0] == "net" && (path[1] == "get" || path[1] == "fetch") {
        let rendered_args: Vec<String> =
            args.iter().map(|a| emit_expr_runtime(a, params)).collect();
        return format!(
            "__cui_net_{}({}, __cui_net_hosts, __cui_cred)",
            path[1],
            rendered_args.join(", ")
        );
    }
    if path.len() == 2 && path[0] == "net" && path[1] == "post" {
        let rendered_args: Vec<String> =
            args.iter().map(|a| emit_expr_runtime(a, params)).collect();
        return format!("__cui_net_post({}, __cui_cred)", rendered_args.join(", "));
    }
    if path.len() == 2 && path[0] == "net" && path[1] == "search" {
        // Tier 9: `net.search(q)` resolves to
        // `{__cui_tag:"Ok",value:[...]}` (or Err) after a
        // simulated network delay.
        let rendered_args: Vec<String> =
            args.iter().map(|a| emit_expr_runtime(a, params)).collect();
        return format!("__cui_net_search({})", rendered_args.join(", "));
    }
    let rendered_args: Vec<String> = args.iter().map(|a| emit_expr_runtime(a, params)).collect();
    format!(
        "/* unsupported call {}.{}({}) */",
        path.join("."),
        "",
        rendered_args.join(", ")
    )
}
