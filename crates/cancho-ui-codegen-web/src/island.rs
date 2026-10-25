//! Sealed islands on the page (docs/islands.md): the `<iframe>` is the IR crate's recipe (`cancho_ui_ir::island::island_dom`), which the prerenderer
//! writes as well; this writes the DOM calls for it, the document of each island once, and the runtime that feeds a frame its props and hears its events.

use cancho_ui_ir::ir::*;

use crate::emit::js_string;

const DOC_MARK: &str = "__CUI_DOC__";

/// Does a view hold an island?
pub(crate) fn view_has_island(node: &IrViewNode) -> bool {
    let mut names = Vec::new();
    islands_in(node, &mut names);
    !names.is_empty()
}

/// The islands a view instantiates, once each, in order.
pub(crate) fn islands_in<'a>(node: &'a IrViewNode, out: &mut Vec<&'a IrIsland>) {
    match node {
        IrViewNode::Element { kind, children, .. } => {
            if let IrElementKind::Island(i) = kind
                && !out.iter().any(|o| o.name == i.name)
            {
                out.push(i);
            }
            for c in children {
                islands_in(c, out);
            }
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            for c in children {
                islands_in(c, out);
            }
        }
        IrViewNode::Text { .. } => {}
    }
}

/// The events the view's islands are heard on, as handler names, once each.
pub(crate) fn handlers_in(node: &IrViewNode, out: &mut Vec<String>) {
    match node {
        IrViewNode::Element {
            kind,
            bindings,
            children,
            ..
        } => {
            if matches!(kind, IrElementKind::Island(_)) {
                for b in bindings {
                    if b.key == "on"
                        && let IrBindingValue::EventRef(n) = &b.value
                        && !out.contains(n)
                    {
                        out.push(n.clone());
                    }
                }
            }
            for c in children {
                handlers_in(c, out);
            }
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            for c in children {
                handlers_in(c, out);
            }
        }
        IrViewNode::Text { .. } => {}
    }
}

/// The props an open island reads from state, as `(island, [(prop, JavaScript that reads it)])`: the page's script gives them to the island at every render.
pub(crate) fn dynamic_in(node: &IrViewNode, out: &mut Vec<(String, Vec<(String, String)>)>) {
    match node {
        IrViewNode::Element {
            kind,
            bindings,
            children,
            ..
        } => {
            if let IrElementKind::Island(i) = kind
                && !i.sealed
            {
                let mut props = Vec::new();
                for b in bindings {
                    if b.key != "on"
                        && b.key != "text"
                        && let IrBindingValue::Const { expr, .. } = &b.value
                        && let IrExpr::Ident { .. } = &**expr
                    {
                        props.push((b.key.clone(), crate::expr::emit_expr_runtime(expr, &[])));
                    }
                }
                if !props.is_empty() {
                    out.push((i.name.clone(), props));
                }
            }
            for c in children {
                dynamic_in(c, out);
            }
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            for c in children {
                dynamic_in(c, out);
            }
        }
        IrViewNode::Text { .. } => {}
    }
}

/// The table of the scripts and styles of the open islands: the page's script runs them, one island at a time, where the island's `div` is.
pub(crate) fn emit_open_sources(islands: &[&IrIsland]) -> String {
    let entries: Vec<String> = islands
        .iter()
        .filter(|i| !i.sealed)
        .map(|i| {
            let scripts: Vec<String> = i.texts.iter().map(|t| js_string(t)).collect();
            format!(
                "{}: {{ style: {}, scripts: [{}] }}",
                js_string(&i.name),
                i.style_text
                    .as_deref()
                    .map_or("null".to_string(), js_string),
                scripts.join(", ")
            )
        })
        .collect();
    if entries.is_empty() {
        String::new()
    } else {
        format!("const __cui_open_src = {{ {} }};\n", entries.join(", "))
    }
}

pub(crate) fn doc_const(name: &str) -> String {
    format!("__cui_doc_{name}")
}

/// The module-level constant that holds the document of an island.
pub(crate) fn emit_doc(i: &IrIsland) -> String {
    format!("const {} = {};\n", doc_const(&i.name), js_string(&i.srcdoc))
}

/// The statements that build one instance into `parent_var`: the frame, with the island's document from its constant.
pub(crate) fn emit(
    indent: &str,
    var: &str,
    parent_var: &str,
    island: &IrIsland,
    bindings: &[IrBinding],
) -> String {
    let mut dom = cancho_ui_ir::island::island_dom(island, bindings);
    for (k, v) in &mut dom.attrs {
        if *k == "srcdoc" {
            *v = DOC_MARK.to_string();
        }
    }
    let mut out = String::new();
    crate::media::emit_dom(indent, var, &dom, parent_var, &mut out);
    out.replace(&js_string(DOC_MARK), &doc_const(&island.name))
}

/// The call after a render that gives every frame of the component its handlers.
pub(crate) fn emit_scan(
    indent: &str,
    handlers: &[String],
    dynamic: &[(String, Vec<(String, String)>)],
) -> String {
    let dyn_table: Vec<String> = dynamic
        .iter()
        .map(|(name, props)| {
            let ps: Vec<String> = props
                .iter()
                .map(|(k, e)| format!("{}: {e}", js_string(k)))
                .collect();
            format!("{}: {{ {} }}", js_string(name), ps.join(", "))
        })
        .collect();
    let table: Vec<String> = handlers
        .iter()
        .map(|h| {
            format!(
                "{}: function () {{ return __on_{h}.apply(null, arguments); }}",
                js_string(h)
            )
        })
        .collect();
    format!(
        "{indent}__cui_islands_scan(root, {{ {} }}, {{ {} }});\n",
        table.join(", "),
        dyn_table.join(", ")
    )
}

/// The runtime of sealed islands. One `message` listener hears every frame; a frame is known by its window, never by what it says about itself.
/// A frame is fed the whole current props when it says `ready` and when the page registers it, and again when the pointer moves (at most
/// `POINTER_MAX_PER_SECOND` a second, only while the frame is on screen). An event is heard when it is one the island declares and the
/// view hears, its value has the declared type, and the frame has not sent more than 50 in the last second; anything else is dropped and counted.
pub(crate) const ADAPTER_JS: &str = r#"const __cui_isl = { live: [], seen: new WeakMap(), ptr: null, last: 0, timer: 0, stats: { sent: 0, pointer: 0, heard: 0, dropped: 0 } };
window.__cui_island_stats = __cui_isl.stats;
function __cui_isl_post(en) {
    const w = en.frame.contentWindow;
    if (!w) { return; }
    w.postMessage({ cui: 'props', props: en.props }, '*');
    __cui_isl.stats.sent++;
}
function __cui_isl_ok(type, v) {
    if (type === 'Int') { return typeof v === 'number' && Number.isInteger(v); }
    if (type === 'Float') { return typeof v === 'number' && isFinite(v); }
    if (type === 'Bool') { return typeof v === 'boolean'; }
    if (type === 'String') { return typeof v === 'string' && v.length <= 4096; }
    return v === undefined || v === null;
}
window.addEventListener('message', function (e) {
    const d = e.data;
    if (!d || typeof d !== 'object') { return; }
    let en = null;
    for (let i = 0; i < __cui_isl.live.length; i++) { if (!__cui_isl.live[i].open && __cui_isl.live[i].frame.contentWindow === e.source) { en = __cui_isl.live[i]; break; } }
    if (!en) { return; }
    if (d.cui === 'ready') { en.ready = true; __cui_isl_post(en); return; }
    if (d.cui === 'pointer') {
        // the pointer over a frame is the frame's, not the page's: the frame says where it is, in its own pixels
        if (!en.spec.pointer || typeof d.x !== 'number' || typeof d.y !== 'number' || !isFinite(d.x) || !isFinite(d.y)) { __cui_isl.stats.dropped++; return; }
        const r = en.frame.getBoundingClientRect();
        __cui_isl_move(r.left + d.x, r.top + d.y);
        return;
    }
    if (d.cui !== 'emit') { return; }
    __cui_isl_event(en, d.name, d.value);
});
function __cui_isl_event(en, name, value) {
    const now = Date.now();
    if (now - en.window0 >= 1000) { en.window0 = now; en.count = 0; }
    const decl = en.spec.events[name];
    if (++en.count > 50 || typeof name !== 'string' || !Object.prototype.hasOwnProperty.call(en.spec.events, name) || !__cui_isl_ok(decl.type, value) || !en.handlers[name]) {
        __cui_isl.stats.dropped++;
        return;
    }
    __cui_isl.stats.heard++;
    try {
        const r = decl.type === null ? en.handlers[name]() : en.handlers[name](value);
        if (r && r.catch) { r.catch(function () {}); }
    } catch (x) {}
}
function __cui_isl_pointer() {
    __cui_isl.last = Date.now();
    __cui_isl.timer = 0;
    const p = __cui_isl.ptr;
    if (!p) { return; }
    for (let i = 0; i < __cui_isl.live.length; i++) {
        const en = __cui_isl.live[i];
        if (en.spec.pointer && en.visible) {
            en.props[en.spec.pointer] = p;
            __cui_isl_post(en);
            __cui_isl.stats.pointer++;
        }
    }
}
function __cui_isl_move(x, y) {
    const c = function (n) { return n < 0 ? 0 : n > 1 ? 1 : n; };
    __cui_isl.ptr = { x: c(x / Math.max(1, window.innerWidth)), y: c(y / Math.max(1, window.innerHeight)) };
    const wait = 50 - (Date.now() - __cui_isl.last);
    if (wait <= 0) { __cui_isl_pointer(); } else if (!__cui_isl.timer) { __cui_isl.timer = setTimeout(__cui_isl_pointer, wait); }
}
let __cui_isl_listening = false;
function __cui_isl_listen() {
    if (__cui_isl_listening) { return; }
    __cui_isl_listening = true;
    window.addEventListener('pointermove', function (ev) { __cui_isl_move(ev.clientX, ev.clientY); }, { passive: true });
}
// An open island (docs/islands.md section 12) runs in the page: its scripts are run from the text the compiler checked against their hashes, each
// in a function that is given `cancho` ({ host, props, onprops, emit, ready }), and nothing sandboxes them.
function __cui_isl_open_run(en) {
    const src = typeof __cui_open_src === 'undefined' ? null : __cui_open_src[en.spec.name];
    if (!src) { return; }
    if (src.style && !document.querySelector('style[data-cui-island-style="' + en.spec.name + '"]')) {
        const st = document.createElement('style');
        st.setAttribute('data-cui-island-style', en.spec.name);
        st.textContent = src.style;
        document.head.appendChild(st);
    }
    const listeners = [];
    const api = {
        host: en.frame,
        props: en.props,
        onprops: function (f) { if (typeof f === 'function') { listeners.push(f); try { f(api.props); } catch (x) {} } },
        emit: function (name, value) { __cui_isl_event(en, String(name), value); },
        ready: function () {}
    };
    en.api = api;
    en.listeners = listeners;
    window.__cui_open_cur = api;
    for (let i = 0; i < src.scripts.length; i++) {
        const s = document.createElement('script');
        s.textContent = '(function (cancho) {\n' + src.scripts[i] + '\n})(window.__cui_open_cur);';
        document.head.appendChild(s);
        s.remove();
    }
    window.__cui_open_cur = null;
}
function __cui_isl_open_sync(en, vals) {
    let changed = false;
    for (const k in vals) {
        if (JSON.stringify(en.props[k]) !== JSON.stringify(vals[k])) { en.props[k] = vals[k]; changed = true; }
    }
    if (changed && en.api) {
        en.api.props = en.props;
        en.listeners.slice().forEach(function (f) { try { f(en.props); } catch (x) {} });
    }
}
function __cui_islands_scan(root, handlers, dynamic) {
    __cui_isl.live = __cui_isl.live.filter(function (en) { return en.frame.isConnected; });
    const frames = root.querySelectorAll('[data-cui-island]');
    for (let i = 0; i < frames.length; i++) {
        const f = frames[i];
        let en = __cui_isl.seen.get(f);
        if (en) {
            en.handlers = handlers;
            if (en.open) { __cui_isl_open_sync(en, dynamic[en.spec.name] || {}); }
            continue;
        }
        let spec;
        try { spec = JSON.parse(f.getAttribute('data-cui-island')); } catch (x) { continue; }
        if (spec.open) {
            en = { frame: f, spec: spec, props: Object.assign({}, spec.props, dynamic[spec.name] || {}), handlers: handlers, open: true, window0: 0, count: 0 };
            __cui_isl.seen.set(f, en);
            __cui_isl.live.push(en);
            __cui_isl_open_run(en);
            continue;
        }
        en = { frame: f, spec: spec, props: Object.assign({}, spec.props), handlers: handlers, ready: false, visible: typeof IntersectionObserver === 'undefined', window0: 0, count: 0 };
        if (spec.pointer) {
            en.props[spec.pointer] = { x: 0.5, y: 0.5 };
            __cui_isl_listen();
            if (typeof IntersectionObserver !== 'undefined') {
                new IntersectionObserver(function (rs) { en.visible = rs[rs.length - 1].isIntersecting; }).observe(f);
            }
        }
        __cui_isl.seen.set(f, en);
        __cui_isl.live.push(en);
        __cui_isl_post(en);
    }
}
"#;
