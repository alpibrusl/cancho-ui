//! The `<meta http-equiv="Content-Security-Policy">` of every page the compiler writes (`--page` and `--pwa`; docs/security-model.md, docs/maps.md, docs/structure.md 12.0r).
//!
//! A page used to have a policy only when its program drew a map or held a sealed island, so taking the map out of a page took the page's only policy with it. Now every page has
//! the **baseline** below, and a program adds to it what it declared:
//!
//! * `default-src 'self'`: nothing is loaded from another origin unless a directive says so.
//! * `script-src 'self'` and the SHA-256 of each inline `<script>` of the page (the sticky header's measure, the worker's registration, the hash scroll): a script an attacker puts
//!   in the page is not one of them. A page with a sealed island also lists the hashes of the scripts of the island's frame, because a `srcdoc` frame inherits the page's policy.
//! * `style-src 'self' 'unsafe-inline'`: the page writes `<style>` blocks and `style` attributes (a frame's aspect ratio, a bar's width). Inline style is the one inline thing allowed.
//! * `img-src 'self' data:`: the pictures of the site, and the `data:` address the page's own stylesheet uses for the faint pattern behind a chat. A map adds its declared tile hosts.
//!   A `Photo`, a `face(..)` and an `Icon` are drawn on a canvas or as inline SVG and need no entry.
//! * `connect-src 'self'` and each `requires Network("host")` the program declares: the adapter (`net.get`) already refuses any other host; this is the browser doing it too.
//! * `object-src 'none'` and `base-uri 'self'`.
//!
//! A directive of the program with the name of one of the baseline adds its sources to it (order kept, no duplicates); any other directive (`frame-src about:` of a sealed island) is
//! added as it is.

use cancho_ui_ir::ir::IrFile;

use crate::theme::sha256_hex;

/// The sources of the page's inline scripts: `'sha256-<base64>'` for the body of each `<script>` (no attributes) of `html`, in order, without duplicates. A script inside an attribute
/// (the `srcdoc` of a frame) is escaped there and is not found; `program_directives` hashes those from the island's own document.
pub fn inline_script_hashes(html: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = html;
    while let Some(i) = rest.find("<script>") {
        let body = &rest[i + "<script>".len()..];
        let Some(end) = body.find("</script>") else {
            break;
        };
        let h = cancho_ui_ir::island::csp_hash(&sha256_hex(&body.as_bytes()[..end]));
        if !out.contains(&h) {
            out.push(h);
        }
        rest = &body[end + "</script>".len()..];
    }
    out
}

/// What the program adds to the baseline, as a policy text (`None`: nothing): the `img-src` of its maps, the `frame-src about:` of a sealed island and the hashes of the island's scripts, the
/// `connect-src` of the hosts it declared.
pub fn program_directives(ir: Option<&IrFile>) -> Option<String> {
    let ir = ir?;
    let mut parts: Vec<String> = Vec::new();
    if let Some(p) = cancho_ui_ir::island::page_policy(ir) {
        parts.push(p);
    }
    let mut hosts: Vec<String> = Vec::new();
    for c in &ir.components {
        for r in c.requires.iter().filter(|r| r.name == "Network") {
            for b in &r.bounds {
                let h = b.to_ascii_lowercase();
                if !hosts.contains(&h) {
                    hosts.push(h);
                }
            }
        }
    }
    if !hosts.is_empty() {
        parts.push(format!("connect-src 'self' {}", hosts.join(" ")));
    }
    let mut frame_hashes: Vec<String> = Vec::new();
    for c in &ir.components {
        collect_frames(&c.view.body, &mut frame_hashes);
    }
    if !frame_hashes.is_empty() {
        parts.push(format!("script-src 'self' {}", frame_hashes.join(" ")));
    }
    (!parts.is_empty()).then(|| parts.join("; "))
}

fn collect_frames(node: &cancho_ui_ir::ir::IrViewNode, out: &mut Vec<String>) {
    use cancho_ui_ir::ir::{IrElementKind, IrViewNode};
    match node {
        IrViewNode::Element { kind, children, .. } => {
            if let IrElementKind::Island(i) = kind
                && i.sealed
            {
                for h in inline_script_hashes(&i.srcdoc) {
                    if !out.contains(&h) {
                        out.push(h);
                    }
                }
            }
            for c in children {
                collect_frames(c, out);
            }
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            for c in children {
                collect_frames(c, out);
            }
        }
        IrViewNode::Text { .. } => {}
    }
}

/// The policy of a page: the baseline (with the hashes of `html`'s inline scripts) with `extra`, the program's directives, added to it.
pub fn policy(extra: Option<&str>, html: &str) -> String {
    let mut script = vec!["'self'".to_string()];
    script.extend(inline_script_hashes(html));
    let mut directives: Vec<(String, Vec<String>)> = vec![
        ("default-src".into(), vec!["'self'".into()]),
        ("script-src".into(), script),
        (
            "style-src".into(),
            vec!["'self'".into(), "'unsafe-inline'".into()],
        ),
        ("img-src".into(), vec!["'self'".into(), "data:".into()]),
        ("connect-src".into(), vec!["'self'".into()]),
        ("object-src".into(), vec!["'none'".into()]),
        ("base-uri".into(), vec!["'self'".into()]),
    ];
    for d in extra.unwrap_or("").split(';') {
        let mut words = d.split_whitespace();
        let Some(name) = words.next() else { continue };
        let at = match directives.iter().position(|(n, _)| n == name) {
            Some(i) => i,
            None => {
                directives.push((name.to_string(), Vec::new()));
                directives.len() - 1
            }
        };
        for w in words {
            if !directives[at].1.iter().any(|s| s == w) {
                directives[at].1.push(w.to_string());
            }
        }
    }
    directives
        .iter()
        .map(|(n, s)| format!("{n} {}", s.join(" ")))
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_baseline_is_there_with_nothing_declared() {
        let p = policy(None, "<html></html>");
        assert_eq!(
            p,
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'self'"
        );
    }

    #[test]
    fn inline_scripts_are_listed_by_hash_and_a_script_with_a_src_is_not() {
        let html = "<script type=\"module\" src=\"./app.js\"></script><script>a()</script><script>a()</script><script>b()</script>";
        let p = policy(None, html);
        assert_eq!(p.matches("'sha256-").count(), 2, "{p}");
        let script = p
            .split("; ")
            .find(|d| d.starts_with("script-src"))
            .expect("a script-src");
        assert!(!script.contains("unsafe-inline"), "{script}");
        // the escaped copy in an attribute is not a script of the page
        assert!(
            inline_script_hashes("<iframe srcdoc=\"&lt;script&gt;x()&lt;/script&gt;\">").is_empty()
        );
    }

    #[test]
    fn a_program_that_adds_scripts_keeps_the_pages_own() {
        let html = "<script>page()</script>";
        let own = inline_script_hashes(html).remove(0);
        let p = policy(Some("script-src 'self' 'sha256-frame'"), html);
        let script = p
            .split("; ")
            .find(|d| d.starts_with("script-src"))
            .expect("a script-src");
        assert_eq!(
            script,
            format!("script-src 'self' {own} 'sha256-frame'"),
            "{p}"
        );
    }

    #[test]
    fn the_program_adds_to_a_directive_and_brings_its_own() {
        let p = policy(
            Some(
                "img-src 'self' data: https://tiles.example; frame-src about:; connect-src 'self' api.example",
            ),
            "",
        );
        assert!(
            p.contains("img-src 'self' data: https://tiles.example;"),
            "{p}"
        );
        assert!(p.contains("frame-src about:"), "{p}");
        assert!(p.contains("connect-src 'self' api.example;"), "{p}");
        assert_eq!(p.matches("img-src").count(), 1, "{p}");
    }
}
