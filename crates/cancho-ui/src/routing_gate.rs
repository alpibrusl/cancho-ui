//! A program that uses routing (docs/routing.md) can be checked, replayed, inspected and diffed, but the page for it is not
//! written yet (the next build step of #75). Until it is, `build` says so with a rule tag instead of emitting a page that
//! would draw no link and never run its route event. Removed with that step.

use std::path::Path;

use cancho_ui_ir::ir::{IrElementKind, IrFile, IrScheduleKind, IrViewNode};

use crate::pipeline::{
    Confidence, Diagnostic, PipelineError, Remediation, Severity, SpanOut, Subject, Tier,
};

fn uses_navlink(node: &IrViewNode) -> bool {
    match node {
        IrViewNode::Element { kind, children, .. } => {
            matches!(kind, IrElementKind::NavLink) || children.iter().any(uses_navlink)
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            children.iter().any(uses_navlink)
        }
        IrViewNode::Text { .. } => false,
    }
}

/// `Err` with `routing-not-emitted` when a component routes.
pub fn refuse_routing(ir: &IrFile, file: &Path) -> Result<(), PipelineError> {
    for c in &ir.components {
        let route = c
            .schedules
            .iter()
            .find(|s| s.kind == IrScheduleKind::Route)
            .map(|s| s.span);
        let router_call = c
            .reachable_effects
            .iter()
            .find(|e| e.capability == "Router")
            .map(|e| e.span);
        let link = uses_navlink(&c.view.body).then_some(c.view.span);
        if let Some(span) = route.or(router_call).or(link) {
            return Err(PipelineError::Refused(Box::new(Diagnostic {
                code: "routing-not-emitted".into(),
                severity: Severity::Error,
                tier: Tier::Static,
                subject: Subject::Component {
                    name: c.name.clone(),
                },
                message: format!(
                    "component `{}` uses routing (`route`, `router.go`/`router.replace` or `NavLink`), which `check`, `test`, `inspect` and `diff` support and `build` does not emit yet (docs/routing.md section 11, step 2)",
                    c.name
                ),
                span: Some(SpanOut::from_span(span, file)),
                expected: None,
                actual: None,
                remediation: Some(Remediation {
                    description: "Check and replay the program with `cancho-ui check` and `cancho-ui test`; the page is the next step of #75.".into(),
                    suggestion: None,
                    confidence: Confidence::High,
                }),
                related: Vec::new(),
                notes: Vec::new(),
            })));
        }
    }
    Ok(())
}
