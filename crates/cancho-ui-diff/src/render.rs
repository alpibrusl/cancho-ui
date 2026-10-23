//! Span-free, canonical text for IR fragments. Two fragments that
//! differ only in where they sit in the source render identically,
//! which is what lets the diff ignore line shifts and comments.
//! Nothing here evaluates anything: `1 + 1` and `2` render
//! differently on purpose (docs/semantic-diff.md §1.2).

use cancho_ui_ir::ir::*;

pub fn ty(t: &IrType) -> String {
    match t {
        IrType::Int => "Int".into(),
        IrType::Float => "Float".into(),
        IrType::Bool => "Bool".into(),
        IrType::String => "String".into(),
        IrType::Result { ok, err } => format!("Result[{}, {}]", ty(ok), ty(err)),
        IrType::Request { payload } => format!("Request[{}]", ty(payload)),
        IrType::Option { some } => format!("Option[{}]", ty(some)),
        IrType::List { elem } => format!("List[{}]", ty(elem)),
    }
}

fn bin_op(op: IrBinOp) -> &'static str {
    match op {
        IrBinOp::Add => "+",
        IrBinOp::Sub => "-",
        IrBinOp::Mul => "*",
        IrBinOp::Div => "/",
        IrBinOp::Eq => "==",
        IrBinOp::Ne => "!=",
        IrBinOp::Lt => "<",
        IrBinOp::Le => "<=",
        IrBinOp::Gt => ">",
        IrBinOp::Ge => ">=",
        IrBinOp::And => "&&",
        IrBinOp::Or => "||",
    }
}

fn pattern(p: &IrPattern) -> String {
    match p {
        IrPattern::Wildcard { .. } => "_".into(),
        IrPattern::Ident { name, .. } => name.clone(),
        IrPattern::Variant { name, binding, .. } => format!("{name}({binding})"),
        IrPattern::UnitVariant { name, .. } => name.clone(),
    }
}

pub fn expr(e: &IrExpr) -> String {
    match e {
        IrExpr::IntLit { value, .. } => value.to_string(),
        IrExpr::FloatLit { value, .. } => format!("{value:?}"),
        IrExpr::StringLit { value, .. } => format!("{value:?}"),
        IrExpr::BoolLit { value, .. } => value.to_string(),
        IrExpr::Ident { name, .. } => name.clone(),
        IrExpr::Call { path, args, .. } => {
            let args: Vec<String> = args.iter().map(expr).collect();
            format!("{}({})", path.join("."), args.join(", "))
        }
        IrExpr::Binary { op, lhs, rhs, .. } => {
            format!("({} {} {})", expr(lhs), bin_op(*op), expr(rhs))
        }
        IrExpr::Unary { op, operand, .. } => match op {
            IrUnOp::Neg => format!("-{}", expr(operand)),
            IrUnOp::Not => format!("!{}", expr(operand)),
        },
        IrExpr::If {
            cond, then, else_, ..
        } => format!(
            "if {} {{ {} }} else {{ {} }}",
            expr(cond),
            expr(then),
            expr(else_)
        ),
        IrExpr::Match {
            scrutinee, arms, ..
        } => {
            let arms: Vec<String> = arms
                .iter()
                .map(|a| format!("{} => {}", pattern(&a.pattern), expr(&a.body)))
                .collect();
            format!("match {} {{ {} }}", expr(scrutinee), arms.join(", "))
        }
        IrExpr::Ok { value, .. } => format!("Ok({})", expr(value)),
        IrExpr::Err { reason, .. } => format!("Err({})", expr(reason)),
        IrExpr::Await { request, .. } => format!("await({})", expr(request)),
        IrExpr::Cancelled { .. } => "Cancelled".into(),
        IrExpr::Some { value, .. } => format!("Some({})", expr(value)),
        IrExpr::None { .. } => "None".into(),
        IrExpr::ListLit { items, .. } => {
            let items: Vec<String> = items.iter().map(expr).collect();
            format!("[{}]", items.join(", "))
        }
        IrExpr::Filter {
            list, item, cond, ..
        } => format!("filter({}, {} => {})", expr(list), item, expr(cond)),
        IrExpr::Map {
            list, item, body, ..
        } => format!("map({}, {} => {})", expr(list), item, expr(body)),
        IrExpr::SortBy {
            list, item, key, ..
        } => format!("sort({}, {} => {})", expr(list), item, expr(key)),
        IrExpr::Len { list, .. } => format!("len({})", expr(list)),
    }
}

pub fn stmt(s: &IrStmt) -> String {
    match s {
        IrStmt::Assign { target, value, .. } => format!("{target} = {};", expr(value)),
        IrStmt::Expr { expr: e, .. } => format!("{};", expr(e)),
        IrStmt::If {
            cond, then, else_, ..
        } => {
            let body = |stmts: &[IrStmt]| stmts.iter().map(stmt).collect::<Vec<_>>().join(" ");
            if else_.is_empty() {
                format!("if {} {{ {} }}", expr(cond), body(then))
            } else {
                format!(
                    "if {} {{ {} }} else {{ {} }}",
                    expr(cond),
                    body(then),
                    body(else_)
                )
            }
        }
    }
}

fn role(r: &IrRole) -> String {
    match r {
        IrRole::Text => "text".into(),
        IrRole::Heading => "heading".into(),
        IrRole::Button => "button".into(),
        IrRole::Other(s) => s.clone(),
    }
}

fn kind(k: &IrElementKind) -> String {
    match k {
        IrElementKind::Column => "Column".into(),
        IrElementKind::Row => "Row".into(),
        IrElementKind::Table => "Table".into(),
        IrElementKind::TableRow => "TableRow".into(),
        IrElementKind::Cell => "Cell".into(),
        IrElementKind::HeaderCell => "HeaderCell".into(),
        IrElementKind::Text => "Text".into(),
        IrElementKind::Button => "Button".into(),
        IrElementKind::TextField => "TextField".into(),
        IrElementKind::TextArea => "TextArea".into(),
        IrElementKind::Checkbox => "Checkbox".into(),
        IrElementKind::Select => "Select".into(),
        IrElementKind::Named(n) => n.clone(),
    }
}

fn binding(b: &IrBinding) -> String {
    let v = match &b.value {
        IrBindingValue::String(s) => format!("{s:?}"),
        IrBindingValue::EventRef(e) => e.clone(),
        IrBindingValue::EventCall { name, args } => {
            let args: Vec<String> = args.iter().map(expr).collect();
            format!("{name}({})", args.join(", "))
        }
        IrBindingValue::Role(r) => role(r),
        IrBindingValue::StateRef(s) => s.clone(),
        IrBindingValue::Const { expr: e, .. } => expr(e),
    };
    format!("{}={}", b.key, v)
}

/// One line per element in the view tree (pre-order), describing what a
/// reader of the page or an assistive technology can observe and which
/// event or state each control is bound to. Text nodes are not elements
/// and are not listed (docs/semantic-diff.md §8).
pub fn view_descriptors(node: &IrViewNode, out: &mut Vec<String>) {
    match node {
        IrViewNode::Element {
            kind: k,
            accessibility,
            bindings,
            children,
            ..
        } => {
            let mut d = kind(k);
            if let Some(r) = &accessibility.role {
                d.push_str(&format!(" role={}", role(r)));
            }
            if let Some(l) = &accessibility.label {
                d.push_str(&format!(" label={l:?}"));
            }
            if let Some(l) = &accessibility.label_expr {
                d.push_str(&format!(" label={}", expr(l)));
            }
            let mut bs: Vec<String> = bindings.iter().map(binding).collect();
            bs.sort();
            for b in bs {
                d.push(' ');
                d.push_str(&b);
            }
            out.push(d);
            for c in children {
                view_descriptors(c, out);
            }
        }
        IrViewNode::Text { .. } => {}
        IrViewNode::For {
            over,
            binding,
            children,
            ..
        } => {
            out.push(format!("For over={} as={binding}", expr(over)));
            for c in children {
                view_descriptors(c, out);
            }
        }
        IrViewNode::If { cond, children, .. } => {
            out.push(format!("If when={}", expr(cond)));
            for c in children {
                view_descriptors(c, out);
            }
        }
    }
}
