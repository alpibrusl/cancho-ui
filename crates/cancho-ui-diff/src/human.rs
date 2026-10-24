//! The short human form. Parallel to the JSON, never a second source of
//! truth: every line is derived from a `DiffReport` field.

use std::fmt::Write;

use crate::{Change, ComponentDiff, DiffReport};

fn change(c: &Change<String>) -> String {
    format!("{} -> {}", c.base, c.candidate)
}

fn component(out: &mut String, d: &ComponentDiff) {
    let _ = writeln!(
        out,
        "component {}  (changed; authority {})",
        d.name,
        d.authority.as_str()
    );
    for r in &d.requires.added {
        let _ = writeln!(out, "  + requires {r}");
    }
    for r in &d.requires.removed {
        let _ = writeln!(out, "  - requires {r}");
    }
    for s in &d.state.added {
        let _ = writeln!(out, "  + state {}: {} = {}", s.name, s.ty, s.initial);
    }
    for s in &d.state.removed {
        let _ = writeln!(out, "  - state {}: {} = {}", s.name, s.ty, s.initial);
    }
    for s in &d.state.changed {
        if let Some(t) = &s.ty {
            let _ = writeln!(out, "  ~ state {}: type {}", s.name, change(t));
        }
        if let Some(i) = &s.initial {
            let _ = writeln!(out, "  ~ state {}: initial {}", s.name, change(i));
        }
    }
    for e in &d.events.added {
        let _ = writeln!(
            out,
            "  + event {}({}): writes {}",
            e.name,
            e.params.join(", "),
            list(&e.writes)
        );
    }
    for e in &d.events.removed {
        let _ = writeln!(
            out,
            "  - event {}({}): writes {}",
            e.name,
            e.params.join(", "),
            list(&e.writes)
        );
    }
    for e in &d.events.changed {
        let mut parts = Vec::new();
        if let Some(p) = &e.params {
            parts.push(format!(
                "params ({}) -> ({})",
                p.base.join(", "),
                p.candidate.join(", ")
            ));
        }
        for w in &e.writes_added {
            parts.push(format!("writes +{w}"));
        }
        for w in &e.writes_removed {
            parts.push(format!("writes -{w}"));
        }
        if e.body.is_some() {
            parts.push("body changed".to_string());
        }
        let _ = writeln!(out, "  ~ event {}: {}", e.name, parts.join("; "));
    }
    for e in &d.effects.added {
        let at: Vec<String> = e
            .spans
            .iter()
            .map(|s| format!("{}:{}", s.start.line, s.start.column))
            .collect();
        let _ = writeln!(
            out,
            "  + effect {} {} at {}",
            e.capability,
            e.path,
            at.join(", ")
        );
    }
    for e in &d.effects.removed {
        let at: Vec<String> = e
            .spans
            .iter()
            .map(|s| format!("{}:{}", s.start.line, s.start.column))
            .collect();
        let _ = writeln!(
            out,
            "  - effect {} {} at {} (in base)",
            e.capability,
            e.path,
            at.join(", ")
        );
    }
    for e in &d.effects.changed {
        let _ = writeln!(
            out,
            "  ~ effect {} {}: {} sites -> {}",
            e.capability, e.path, e.base_count, e.candidate_count
        );
    }
    for s in &d.schedules.added {
        let _ = writeln!(out, "  + schedule {s}");
    }
    for s in &d.schedules.removed {
        let _ = writeln!(out, "  - schedule {s}");
    }
    for l in &d.links.added {
        let _ = writeln!(out, "  + link {l}");
    }
    for l in &d.links.removed {
        let _ = writeln!(out, "  - link {l}");
    }
    for v in &d.view.added {
        let _ = writeln!(out, "  + view {v}");
    }
    for v in &d.view.removed {
        let _ = writeln!(out, "  - view {v}");
    }
}

fn list(v: &[String]) -> String {
    if v.is_empty() {
        "nothing".into()
    } else {
        v.join(", ")
    }
}

pub fn render_human(r: &DiffReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "cancho-ui diff {} -> {}", r.base, r.candidate);
    if !r.changed {
        out.push_str("no semantic change\n");
        return out;
    }
    let _ = writeln!(out, "authority: {}", r.authority.as_str());
    for c in &r.components.added {
        let _ = writeln!(
            out,
            "+ component {}  (requires {})",
            c.name,
            list(&c.requires)
        );
    }
    for c in &r.components.removed {
        let _ = writeln!(
            out,
            "- component {}  (requires {})",
            c.name,
            list(&c.requires)
        );
    }
    for d in &r.components.changed {
        component(&mut out, d);
    }
    for t in &r.theme {
        let _ = match (&t.base, &t.candidate) {
            (Some(b), Some(c)) => writeln!(out, "~ theme {}: {b} -> {c}", t.setting),
            (None, Some(c)) => writeln!(out, "+ theme {} = {c}", t.setting),
            (Some(b), None) => writeln!(out, "- theme {} = {b}", t.setting),
            (None, None) => Ok(()),
        };
    }
    for a in &r.assets {
        let short = |h: &String| h.chars().take(12).collect::<String>();
        let _ = match (&a.base, &a.candidate) {
            (Some(b), Some(c)) => writeln!(out, "~ file {}: {} -> {}", a.path, short(b), short(c)),
            (None, Some(c)) => writeln!(out, "+ file {} = {}", a.path, short(c)),
            (Some(b), None) => writeln!(out, "- file {} = {}", a.path, short(b)),
            (None, None) => Ok(()),
        };
    }
    out
}
