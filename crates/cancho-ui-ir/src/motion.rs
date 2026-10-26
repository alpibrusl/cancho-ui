//! Motion (docs/motion.md): `Reveal`, `CountUp` and the `visible:` / `hidden:` events of a section. This module owns the rule tags that need more than
//! one binding, the text a `CountUp` is made of, and the program-wide check that an element that tells the program it was seen is in the page's HTML.
//! What each does in a browser is the page's inline script (`crates/cancho-ui/src/motion.rs`); nothing here is a capability.

use cancho_ui_syntax::Span;

use crate::ir::*;
use crate::lower::LowerError;

/// The largest number a `CountUp` counts up to.
pub const COUNT_MAX: i64 = 1_000_000_000;

fn refuse(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

/// The children of a `CountUp`: the text of its final number, which is what the page holds before, during and after it counts, and what a screen
/// reader reads. `to:` is an integer literal from 0 to `COUNT_MAX`, and an author writes no text.
pub fn countup_children(
    bindings: &[IrBinding],
    children: &[IrViewNode],
    span: Span,
) -> Result<Vec<IrViewNode>, LowerError> {
    if !children.is_empty() {
        return Err(refuse(
            "countup-has-children",
            "a CountUp has no text of its own: its text is its final number, `CountUp { to: 1380 }`".to_string(),
            span,
        ));
    }
    let to = bindings.iter().find(|b| b.key == "to");
    let value = to.and_then(|b| match &b.value {
        IrBindingValue::Const { expr, .. } => match **expr {
            IrExpr::IntLit { value, .. } => Some(value),
            _ => None,
        },
        _ => None,
    });
    let Some(n) = value else {
        return Err(refuse(
            "countup-not-number",
            "a CountUp needs `to:`, a whole number written in the program: `CountUp { to: 1380 }`"
                .to_string(),
            to.map_or(span, |b| b.span),
        ));
    };
    if !(0..=COUNT_MAX).contains(&n) {
        return Err(refuse(
            "countup-out-of-range",
            format!("`to: {n}` is outside 0 to {COUNT_MAX}"),
            to.map_or(span, |b| b.span),
        ));
    }
    Ok(vec![IrViewNode::Text {
        parts: vec![IrTextPart::Literal(n.to_string())],
        span,
    }])
}

/// The event a `visible:` or a `hidden:` names.
pub fn event_of(b: &IrBinding) -> Option<&str> {
    match &b.value {
        IrBindingValue::EventRef(n) => Some(n),
        _ => None,
    }
}

/// The events the elements of a view say they were seen (`visible`) or left (`hidden`) with: sorted, without repeats.
pub fn seen_events(node: &IrViewNode, out: &mut std::collections::BTreeSet<String>) {
    match node {
        IrViewNode::Element {
            bindings, children, ..
        } => {
            for b in bindings {
                if (b.key == "visible" || b.key == "hidden")
                    && let Some(n) = event_of(b)
                {
                    out.insert(n.to_string());
                }
            }
            for c in children {
                seen_events(c, out);
            }
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            for c in children {
                seen_events(c, out);
            }
        }
        IrViewNode::Text { .. } => {}
    }
}

fn count_bindings(node: &IrViewNode) -> usize {
    match node {
        IrViewNode::Element {
            bindings, children, ..
        } => {
            bindings
                .iter()
                .filter(|b| b.key == "visible" || b.key == "hidden")
                .count()
                + children.iter().map(count_bindings).sum::<usize>()
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            children.iter().map(count_bindings).sum()
        }
        IrViewNode::Text { .. } => 0,
    }
}

fn first_binding(node: &IrViewNode) -> Option<Span> {
    match node {
        IrViewNode::Element {
            bindings, children, ..
        } => bindings
            .iter()
            .find(|b| b.key == "visible" || b.key == "hidden")
            .map(|b| b.span)
            .or_else(|| children.iter().find_map(first_binding)),
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            children.iter().find_map(first_binding)
        }
        IrViewNode::Text { .. } => None,
    }
}

/// The program-wide check: every `visible:` and `hidden:` is on an element that is in the page's own HTML, and that no script rebuilds. The page
/// tells the program an element was seen by watching the element it was written with; an element built by script is rebuilt at every render, and
/// the event it fired would draw the page again, which builds the element again.
pub fn check_program(ir: &IrFile) -> Result<(), LowerError> {
    let total: usize = ir
        .components
        .iter()
        .map(|c| count_bindings(&c.view.body))
        .sum();
    if total == 0 {
        return Ok(());
    }
    let (mounted, skeleton) = match crate::recipe::hydrating(ir) {
        Some((i, sk)) => (Some(i), Some(sk)),
        None => (None, None),
    };
    for (i, c) in ir.components.iter().enumerate() {
        let n = count_bindings(&c.view.body);
        if n == 0 {
            continue;
        }
        let in_html = skeleton.as_ref().map_or(0, |sk| {
            sk.html.matches("data-cui-visible=\"").count()
                + sk.html.matches("data-cui-hidden=\"").count()
        });
        if mounted != Some(i) || in_html != n {
            return Err(refuse(
                "visible-dynamic",
                "`visible:` and `hidden:` are on an element that the page's HTML has before any script runs: in the component that is mounted into the page, outside any `For` or `If`. Anything else is drawn by script at every render, and the event would draw it again".to_string(),
                first_binding(&c.view.body).unwrap_or(c.view.span),
            ));
        }
    }
    Ok(())
}
