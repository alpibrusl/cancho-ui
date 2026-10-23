//! `SortList` on the page (docs/drag-and-drop.md): a `<ul>` of rows, each with a drag handle and buttons that move it up and down. The buttons
//! are the keyboard and screen-reader path and the only way a move is made without a pointer; the handle is a pointer drag built on Pointer
//! Events (HTML5 drag and drop is not delivered to a finger and has no keyboard path: measured, `scripts/measure-dnd.js`). While a drag is in
//! progress the page does not rebuild (a rebuild loses the drop); the renders that arrive meanwhile run once, after it.

use cancho_ui_ir::ir::*;

use crate::emit::{STATE_PREFIX, js_string};
use crate::expr::emit_expr_runtime;

/// The statements that draw one `SortList` into `parent_var`. `var` names the list; its text is also the key by which a move finds the
/// list again after the rebuild it causes.
pub(crate) fn emit(
    indent: &str,
    var: &str,
    parent_var: &str,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    params: &[&str],
) -> String {
    let label = match (&accessibility.label, &accessibility.label_expr) {
        (Some(l), _) => js_string(l),
        (None, Some(e)) => format!("String({})", emit_expr_runtime(e, params)),
        (None, None) => "\"\"".to_string(),
    };
    let items = bindings.iter().find_map(|b| match (&*b.key, &b.value) {
        ("items", IrBindingValue::StateRef(name)) => Some(format!("{STATE_PREFIX}{name}")),
        _ => None,
    });
    let on = bindings.iter().find_map(|b| match (&*b.key, &b.value) {
        ("on", IrBindingValue::EventRef(name)) => Some(name.clone()),
        _ => None,
    });
    let (Some(items), Some(on)) = (items, on) else {
        // lowering has refused a SortList without them (`sortlist-incomplete`); an empty list keeps the module valid
        return format!(
            "{indent}const {var} = document.createElement('ul');\n{indent}{parent_var}.appendChild({var});\n"
        );
    };
    format!(
        "{indent}const {var} = __cui_sortlist(root, {key}, {label}, {items}, function (a, b) {{ return __on_{on}(a, b); }});\n{indent}{parent_var}.appendChild({var});\n",
        key = js_string(var)
    )
}

pub(crate) const ADAPTER_JS: &str = r#"// Platform adapter: SortList (docs/drag-and-drop.md). Rows with a handle (a pointer drag) and "move up" / "move down" buttons (the keyboard and
// screen-reader path). The event gets the two indexes; the program changes its list or not, and the page shows what the program holds.
const __cui_drag = { on: false, defer: [] };
function __cui_drag_flush() { const d = __cui_drag.defer; __cui_drag.defer = []; d.forEach(function (f) { f(); }); }
function __cui_sort_style() {
    if (typeof document === 'undefined' || document.getElementById('cui-sort-style')) { return; }
    const s = document.createElement('style');
    s.id = 'cui-sort-style';
    s.textContent = '.cui-sort { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.25rem; }'
        + ' .cui-sort li { display: flex; align-items: center; gap: 0.5rem; min-height: 44px; padding-inline-end: 0.25rem; border: 1px solid color-mix(in srgb, currentColor 25%, transparent); border-radius: 0.25rem; background: Canvas; color: CanvasText; }'
        + ' .cui-sort-text { flex: 1; overflow-wrap: anywhere; }'
        + ' .cui-handle { display: inline-flex; align-items: center; justify-content: center; width: 44px; height: 44px; touch-action: none; cursor: grab; user-select: none; }'
        + ' .cui-dragging { position: relative; z-index: 1; box-shadow: 0 2px 8px rgba(0, 0, 0, 0.35); }'
        + ' .cui-over { outline: 2px solid currentColor; }'
        + ' .cui-sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }';
    document.head.appendChild(s);
}
// One polite live region for the page, outside the tree that is rebuilt (#106), says where a row went.
function __cui_sort_say(text) {
    let r = document.getElementById('cui-sort-live');
    if (!r) {
        r = document.createElement('div');
        r.id = 'cui-sort-live';
        r.className = 'cui-sr';
        r.setAttribute('role', 'status');
        r.setAttribute('aria-live', 'polite');
        document.body.appendChild(r);
    }
    r.textContent = '';
    r.textContent = text;
}
function __cui_sortlist(root, key, label, items, onMove) {
    __cui_sort_style();
    const ul = document.createElement('ul');
    ul.className = 'cui-sort';
    ul.setAttribute('aria-label', label);
    ul.setAttribute('data-cui-key', key);
    const n = items.length;
    // after a move the page has been rebuilt: find the row where it went (the program may also have refused), and say so
    function settle(from, to, which) {
        const now = root.querySelector('[data-cui-key="' + key + '"]');
        if (!now) { return; }
        const text = String(items[from]);
        const accepted = now.children[to] && now.children[to].getAttribute('data-item') === text;
        const at = accepted ? to : from;
        if (accepted) { __cui_sort_say(text + ', position ' + (at + 1) + ' of ' + n); }
        return { list: now, at: at };
    }
    items.forEach(function (item, i) {
        const text = String(item);
        const li = document.createElement('li');
        li.setAttribute('data-item', text);
        const handle = document.createElement('span');
        handle.className = 'cui-handle';
        handle.setAttribute('aria-hidden', 'true');
        handle.textContent = '\u2630';
        const name = document.createElement('span');
        name.className = 'cui-sort-text';
        name.textContent = text;
        const up = document.createElement('button');
        up.textContent = '\u2191';
        up.setAttribute('aria-label', 'Move ' + text + ' up');
        up.disabled = i === 0;
        const down = document.createElement('button');
        down.textContent = '\u2193';
        down.setAttribute('aria-label', 'Move ' + text + ' down');
        down.disabled = i === n - 1;
        // a button move: the focus goes to the same button of the row where it went, so that pressing it again keeps moving the same row
        function viaButton(to, which) {
            onMove(i, to);
            const s = settle(i, to, which);
            if (!s) { return; }
            const btns = s.list.children[s.at].querySelectorAll('button');
            let target = which === 'up' ? btns[0] : btns[1];
            if (target.disabled) { target = which === 'up' ? btns[1] : btns[0]; }
            if (target && !target.disabled) { target.focus(); }
        }
        up.addEventListener('click', function () { viaButton(i - 1, 'up'); });
        down.addEventListener('click', function () { viaButton(i + 1, 'down'); });
        // a pointer drag: mouse, pen or finger; the page is not rebuilt under it
        handle.addEventListener('pointerdown', function (e) {
            if (e.pointerType === 'mouse' && e.button !== 0) { return; }
            e.preventDefault();
            const rows = Array.prototype.slice.call(ul.children);
            const mids = rows.map(function (r) { const b = r.getBoundingClientRect(); return b.top + b.height / 2; });
            const startY = e.clientY;
            let target = i;
            __cui_drag.on = true;
            try { handle.setPointerCapture(e.pointerId); } catch (x) { /* a synthetic pointer */ }
            li.classList.add('cui-dragging');
            function nearest(y) {
                let best = 0;
                for (let k = 1; k < mids.length; k++) { if (Math.abs(mids[k] - y) < Math.abs(mids[best] - y)) { best = k; } }
                return best;
            }
            function onMoveEvt(m) {
                li.style.transform = 'translateY(' + (m.clientY - startY) + 'px)';
                target = nearest(m.clientY);
                rows.forEach(function (r, k) { r.classList.toggle('cui-over', k === target && k !== i); });
            }
            function end(commit) {
                handle.removeEventListener('pointermove', onMoveEvt);
                handle.removeEventListener('pointerup', onUp);
                handle.removeEventListener('pointercancel', onCancel);
                document.removeEventListener('keydown', onKey, true);
                li.classList.remove('cui-dragging');
                li.style.transform = '';
                rows.forEach(function (r) { r.classList.remove('cui-over'); });
                __cui_drag.on = false;
                if (commit && target !== i) { onMove(i, target); settle(i, target, null); }
                __cui_drag_flush();
            }
            function onUp() { end(true); }
            function onCancel() { end(false); }
            function onKey(k) { if (k.key === 'Escape') { k.preventDefault(); end(false); } }
            handle.addEventListener('pointermove', onMoveEvt);
            handle.addEventListener('pointerup', onUp);
            handle.addEventListener('pointercancel', onCancel);
            document.addEventListener('keydown', onKey, true);
        });
        li.appendChild(handle);
        li.appendChild(name);
        li.appendChild(up);
        li.appendChild(down);
        ul.appendChild(li);
    });
    return ul;
}
"#;
