//! `Rail` on the page (docs/structure.md 12.0s). The scrolling and the snapping are the stylesheet's (`scroll-snap-type`, `scroll-snap-align`); the script does only the choosing:
//! it listens on the rail (one listener, not one per child), gives `on:` the place of the child that was tapped, marks the chosen one, moves the focus among the children with the
//! arrow keys, and keeps the rail where it was across the render that every state change is (the page is drawn again from nothing, which would put a scrolled row back at its start).
//! Every string from the program goes in with `setAttribute` or `textContent`.

use cancho_ui_ir::ir::*;

use crate::emit::js_string;
use crate::expr::emit_expr_runtime;

/// The classes and attributes of the box and of the scrolling row inside it, and the options the script is given. Returns `(open, close)`: the code before the children are
/// emitted (the row is `{var}`, it is open for them) and the code after (the box is put in its parent and wired).
pub(crate) fn emit(
    indent: &str,
    var: &str,
    parent: &str,
    key: &str,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    params: &[&str],
) -> (String, String) {
    let vertical = cancho_ui_ir::rail::vertical(bindings);
    let snap = cancho_ui_ir::rail::snap_of(bindings);
    let label = match (&accessibility.label, &accessibility.label_expr) {
        (Some(l), _) => js_string(l),
        (None, Some(e)) => format!("String({})", emit_expr_runtime(e, params)),
        (None, None) => "\"\"".to_string(),
    };
    let event = bindings.iter().find_map(|b| match (&*b.key, &b.value) {
        ("on", IrBindingValue::EventRef(e)) => Some(e.clone()),
        _ => None,
    });
    let selected = bindings
        .iter()
        .find(|b| b.key == "selected")
        .and_then(|b| match &b.value {
            IrBindingValue::Const { expr, .. } => Some(emit_expr_runtime(expr, params)),
            _ => None,
        });
    let gap = bindings
        .iter()
        .find(|b| b.key == "gap")
        .and_then(|b| match &b.value {
            IrBindingValue::Const { expr, .. } => match **expr {
                IrExpr::IntLit { value, .. } => Some(value),
                _ => None,
            },
            _ => None,
        });
    let mut classes = vec![
        "cui-rail".to_string(),
        format!("cui-rail-{snap}"),
        if vertical {
            "cui-rail-v".to_string()
        } else {
            "cui-rail-h".to_string()
        },
    ];
    if let Some(g) = gap {
        classes.push(format!("cui-gap-{g}"));
    }
    let mut open = format!(
        "{indent}const {var}_box = document.createElement('div');\n\
{indent}{var}_box.className = 'cui-rail-box';\n\
{indent}const {var} = document.createElement('div');\n\
{indent}{var}.className = {classes};\n\
{indent}{var}.setAttribute('data-cui-rail', {key});\n\
{indent}{var}.setAttribute('aria-label', {label});\n",
        classes = js_string(&classes.join(" ")),
        key = js_string(key),
    );
    if let Some(w) = cancho_ui_ir::rail::item_width_of(bindings) {
        open.push_str(&format!(
            "{indent}{var}.style.setProperty('--cui-rail-item', '{w}%');\n{indent}{var}.classList.add('cui-rail-sized');\n"
        ));
    }
    let deliver = match &event {
        Some(e) => format!("function (i) {{ __on_{e}(i); }}"),
        None => "null".to_string(),
    };
    let close = format!(
        "{indent}{var}_box.appendChild({var});\n\
{indent}{parent}.appendChild({var}_box);\n\
{indent}__cui_rail_wire({var}_box, {var}, {{ sel: {sel}, on: {deliver}, follow: {follow}, dots: {dots}, vertical: {vertical}, snap: {snap} }});\n",
        sel = selected.unwrap_or_else(|| "-1".to_string()),
        follow = cancho_ui_ir::rail::flag(bindings, "follow"),
        dots = cancho_ui_ir::rail::flag(bindings, "dots"),
        snap = js_string(snap),
    );
    (open, close)
}

/// The script of a rail: the wiring of one, the place of every rail before the page is drawn again, and the putting back after it.
pub(crate) const RAIL_JS: &str = r##"// Builtin: Rail (docs/structure.md 12.0s). The row scrolls and snaps in CSS; this chooses, and keeps the place of the row across a render.
function __cui_rail_dist(el, item, o) {
    // how far the row must scroll for the child to stand where `snap` says (the nearest way to be in view for "none"), in pixels along the row
    var r = el.getBoundingClientRect(), i = item.getBoundingClientRect(), cs = getComputedStyle(el), rtl = !o.vertical && cs.direction === "rtl", a, b, lo, hi, pl, pr;
    if (o.vertical) { a = i.top; b = i.bottom; lo = r.top; hi = r.bottom; pl = parseFloat(cs.scrollPaddingTop) || 0; pr = parseFloat(cs.scrollPaddingBottom) || 0; }
    else { a = i.left; b = i.right; lo = r.left; hi = r.right; pl = parseFloat(cs.scrollPaddingLeft) || 0; pr = parseFloat(cs.scrollPaddingRight) || 0; if (rtl) { var t = pl; pl = pr; pr = t; } }
    if (o.snap === "center") { return (a + b) / 2 - (lo + hi) / 2; }
    if (o.snap === "start") { return rtl ? b - (hi - pl) : a - (lo + pl); }
    if (o.snap === "end") { return rtl ? a - (lo + pr) : b - (hi - pr); }
    return a < lo + pl ? a - (lo + pl) : (b > hi - pr ? b - (hi - pr) : 0);
}
function __cui_rail_reveal(el, item, o, smooth) {
    var d = __cui_rail_dist(el, item, o);
    if (Math.abs(d) < 1) { return; }
    // the scrolling the page does itself must not be heard as the person's, or `follow` would choose each child the row passes
    el.__cui_lock = Date.now() + 800;
    var calm = window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    el.scrollBy({ left: o.vertical ? 0 : d, top: o.vertical ? d : 0, behavior: smooth && !calm ? "smooth" : "auto" });
}
function __cui_rail_wire(box, el, o) {
    var on = o.on, sel = Math.floor(Number(o.sel)), items, focused = 0, dots = null;
    if (!(isFinite(sel) && sel >= 0)) { sel = -1; }
    el.__cui_o = o; el.__cui_sel = sel;
    function kids() { return Array.prototype.slice.call(el.children); }
    function index(target) {
        var t = target;
        while (t && t.parentNode !== el) { t = t.parentNode; }
        return t ? kids().indexOf(t) : -1;
    }
    items = kids();
    if (on) {
        el.setAttribute("role", "listbox");
        el.setAttribute("aria-orientation", o.vertical ? "vertical" : "horizontal");
        focused = sel >= 0 && sel < items.length ? sel : 0;
        items.forEach(function (c, i) {
            c.setAttribute("role", "option");
            c.setAttribute("aria-selected", i === sel ? "true" : "false");
            c.setAttribute("tabindex", i === focused ? "0" : "-1");
            if (i === sel) { c.classList.add("cui-rail-on"); }
        });
    } else {
        // a row with nothing to choose still scrolls by keyboard: the box is a named region the keyboard can reach
        el.setAttribute("role", "group");
        el.setAttribute("tabindex", "0");
    }
    if (o.dots) {
        dots = document.createElement("div");
        dots.className = "cui-rail-dots"; dots.setAttribute("aria-hidden", "true");
        items.forEach(function (c, i) {
            var d = document.createElement("span");
            d.className = i === sel ? "cui-rail-dot cui-rail-dot-on" : "cui-rail-dot";
            dots.appendChild(d);
        });
        box.appendChild(dots);
    }
    if (on) {
        el.addEventListener("click", function (e) {
            var i = index(e.target), t = e.target;
            if (i < 0) { return; }
            // a button, a link or a field inside a child has its own click: choosing the child is for a tap on the child itself
            while (t && t !== el.children[i]) { if (/^(A|BUTTON|INPUT|SELECT|TEXTAREA|LABEL|SUMMARY)$/.test(t.tagName)) { return; } t = t.parentNode; }
            on(i);
        });
        el.addEventListener("keydown", function (e) {
            var i = index(e.target), list = kids(), next = -1, rtl = !o.vertical && getComputedStyle(el).direction === "rtl", fwd = o.vertical ? "ArrowDown" : (rtl ? "ArrowLeft" : "ArrowRight"), back = o.vertical ? "ArrowUp" : (rtl ? "ArrowRight" : "ArrowLeft");
            if (i < 0 || e.target !== list[i] || e.altKey || e.ctrlKey || e.metaKey) { return; }
            if (e.key === "Enter" || e.key === " ") { e.preventDefault(); on(i); return; }
            if (e.key === fwd) { next = Math.min(list.length - 1, i + 1); }
            else if (e.key === back) { next = Math.max(0, i - 1); }
            else if (e.key === "Home") { next = 0; }
            else if (e.key === "End") { next = list.length - 1; }
            else { return; }
            e.preventDefault();
            list[i].setAttribute("tabindex", "-1"); list[next].setAttribute("tabindex", "0");
            list[next].focus({ preventScroll: true });
            __cui_rail_reveal(el, list[next], { vertical: o.vertical, snap: "none" }, true);
        });
    }
    if (on && o.follow) {
        var timer = 0;
        el.addEventListener("scroll", function () {
            if (timer) { clearTimeout(timer); }
            timer = setTimeout(function () {
                var list = kids(), best = -1, bd = Infinity, where = { vertical: o.vertical, snap: o.snap === "none" ? "center" : o.snap };
                if (Date.now() < (el.__cui_lock || 0)) { return; }
                // the child nearest the place `snap:` puts a child (the middle, or an edge), so that the one a row stops on is the one it chooses
                list.forEach(function (c, i) {
                    var d = Math.abs(__cui_rail_dist(el, c, where));
                    if (d < bd) { bd = d; best = i; }
                });
                if (best >= 0 && best !== el.__cui_sel) { on(best); }
            }, 140);
        });
    }
}
// The place of every rail before the page is drawn again, and the child each one had chosen.
function __cui_rail_save(root) {
    var seen = {}, out = {};
    Array.prototype.forEach.call(root.querySelectorAll("[data-cui-rail]"), function (el) {
        var k = el.getAttribute("data-cui-rail"); seen[k] = (seen[k] || 0) + 1;
        out[k + "#" + seen[k]] = { left: el.scrollLeft, top: el.scrollTop, sel: el.__cui_sel };
    });
    return out;
}
// After it: each rail is where it was; a rail whose chosen child changed (a tap, a key, the program) brings that child to the place `snap` says, smoothly; a rail that is new and has a chosen child
// shows it at once.
function __cui_rail_after(root, saved) {
    var seen = {};
    Array.prototype.forEach.call(root.querySelectorAll("[data-cui-rail]"), function (el) {
        var k = el.getAttribute("data-cui-rail"), p, o = el.__cui_o, item;
        if (!o) { return; }
        seen[k] = (seen[k] || 0) + 1;
        p = saved[k + "#" + seen[k]];
        if (p) { el.__cui_lock = Date.now() + 800; el.scrollLeft = p.left; el.scrollTop = p.top; }
        item = el.__cui_sel >= 0 ? el.children[el.__cui_sel] : null;
        if (item && (!p || p.sel !== el.__cui_sel)) { __cui_rail_reveal(el, item, o, !!p); }
    });
}
"##;
