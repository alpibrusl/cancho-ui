//! The JavaScript the emitter copies into a program when the program uses it: the platform adapters (Clock, Storage,
//! Network, Location, cancellation, focus) and the `json.get` builtins. Each is a constant, emitted by `emit_module`
//! only when something in the IR calls it.
/// `json.get(text, path, default)` (docs/language-design.md decision 13): a pure read of one value of a JSON text.
/// Its model is `cancho_ui_replay::eval::json_get`; `tests/json_get_corpus.json` is checked against both.
pub(crate) const JSON_GET_JS: &str = "\
// Builtins: json.get(text, path, default) and json.items(text, path) (docs/language-design.md decision 13). Pure; never throw.
function __cui_json_depth(t) {
    var d = 0, m = 0, s = false;
    for (var i = 0; i < t.length; i++) {
        var c = t.charCodeAt(i);
        if (s) { if (c === 92) { i++; } else if (c === 34) { s = false; } }
        else if (c === 34) { s = true; }
        else if (c === 91 || c === 123) { d++; if (d > m) { m = d; } }
        else if (c === 93 || c === 125) { d--; }
    }
    return m;
}
// The value at `path` of the JSON `text` as {v: value}, or null when there is none.
function __cui_json_at(text, path) {
    var lone = /[\\ud800-\\udbff](?![\\udc00-\\udfff])|(?:^|[^\\ud800-\\udbff])[\\udc00-\\udfff]/;
    try {
        text = String(text);
        path = String(path);
        if (__cui_json_depth(text) > 127) { return null; }
        var cur = JSON.parse(text, function (k, v) {
            if (lone.test(k)) { throw 0; }
            if (typeof v === 'string' && lone.test(v)) { throw 0; }
            if (typeof v === 'number' && !isFinite(v)) { throw 0; }
            return v;
        });
        if (path !== '') {
            var parts = path.split('.');
            for (var i = 0; i < parts.length; i++) {
                var p = parts[i];
                if (Array.isArray(cur)) {
                    if (!/^(0|[1-9][0-9]{0,8})$/.test(p) || Number(p) >= cur.length) { return null; }
                    cur = cur[Number(p)];
                } else if (cur !== null && typeof cur === 'object') {
                    if (!Object.prototype.hasOwnProperty.call(cur, p)) { return null; }
                    cur = cur[p];
                } else {
                    return null;
                }
            }
        }
        return { v: cur };
    } catch (e) {
        return null;
    }
}
function __cui_json_get(text, path, fallback) {
    var at = __cui_json_at(text, path);
    if (at === null) { return fallback; }
    var cur = at.v;
    if (typeof cur === 'string') { return cur; }
    if (typeof cur === 'boolean') { return cur ? 'true' : 'false'; }
    if (typeof cur === 'number' && Number.isInteger(cur) && Math.abs(cur) <= 9007199254740992) { return String(cur); }
    if (typeof cur === 'number' && Math.abs(cur) >= 1e-6 && Math.abs(cur) < 1e15) { return String(cur); }
    return fallback;
}
function __cui_json_items(text, path) {
    var at = __cui_json_at(text, path);
    if (at === null || !Array.isArray(at.v)) { return []; }
    return at.v.slice(0, 1000).map(function (x) { return JSON.stringify(x); });
}
";

/// The `Storage` adapter (docs/adapters.md): a string-to-string store under the key prefix `cui:` in
/// `localStorage`, with a page-lifetime fallback when `localStorage` is absent or throws (a private window,
/// blocked site data, a full quota). `set` answers whether the value is persisted.
pub(crate) const STORAGE_ADAPTER_JS: &str = "\
// Platform adapter: Storage capability (docs/adapters.md).
// Keys live under the prefix `cui:` in localStorage, or `cui:<ns>:` for a component that declares
// `requires Storage(\"ns\")`; a program cannot read or write any other key. When localStorage is
// unavailable (or full) the value lasts as long as the page, `storage.get` returns it, and `storage.set` answers false.
var __cui_storage_memory = {};
function __cui_storage_get(key, fallback, ns) {
    var k = 'cui:' + (ns ? ns + ':' : '') + String(key);
    // A value the store refused is newer than whatever is persisted under the key: the page reads what it last wrote.
    if (Object.prototype.hasOwnProperty.call(__cui_storage_memory, k)) { return __cui_storage_memory[k]; }
    try {
        var v = window.localStorage.getItem(k);
        if (v !== null) { return v; }
    } catch (e) { /* unavailable */ }
    return fallback;
}
function __cui_storage_set(key, value, ns) {
    var k = 'cui:' + (ns ? ns + ':' : '') + String(key);
    try {
        window.localStorage.setItem(k, String(value));
        delete __cui_storage_memory[k];
        return true;
    } catch (e) {
        __cui_storage_memory[k] = String(value);
        return false;
    }
}
";

pub(crate) const CLOCK_ADAPTER_JS: &str = "\
// Platform adapter: Clock capability (Tier 4 stub).
// Phase 2 will replace this with an imported cancho-ui source file
// that wraps `Date.now()` in a typed boundary.
function __cui_clock_now() {
    return Date.now();
}
";

/// Focus survives a re-render. A view is rebuilt from scratch on every state change, so the control the user is
/// using was replaced and focus fell to the page: typing "ada" into a `TextField` left "a", and pressing Enter three
/// times on a focused `Button` counted once (both measured).
/// Before the rebuild the focused control's place under this component's root (its path of child indices) and its
/// text selection (and, for a `TextArea`, its scroll position) are recorded; after it, the control at that path gets both back. A control that is gone, or is a
/// different kind or has another label, stays unfocused rather than focusing the wrong one. Not handled: an
/// input method composition in progress.
pub(crate) const FOCUS_ADAPTER_JS: &str = r#"// Focus survives a re-render: see docs/semantics.md, Question 9.
function __cui_focus_save(root) {
    const el = document.activeElement;
    if (!el || el === document.body || el === root || !root.contains(el)) { return null; }
    const path = [];
    for (let n = el; n !== root; n = n.parentNode) {
        path.unshift(Array.prototype.indexOf.call(n.parentNode.childNodes, n));
    }
    return {
        path: path,
        tag: el.tagName,
        label: el.getAttribute('aria-label'),
        start: el.selectionStart,
        end: el.selectionEnd,
        direction: el.selectionDirection,
        scroll: el.scrollTop,
    };
}
function __cui_focus_restore(root, saved) {
    if (!saved) { return; }
    let n = root;
    for (let i = 0; i < saved.path.length && n; i++) { n = n.childNodes[saved.path[i]]; }
    if (!n || n.tagName !== saved.tag || n.getAttribute('aria-label') !== saved.label) { return; }
    n.focus();
    if (typeof saved.start === 'number' && n.setSelectionRange) {
        try { n.setSelectionRange(saved.start, saved.end, saved.direction); } catch (e) { /* not a text control */ }
    }
    // A multi-line field keeps the lines it was scrolled to (docs/controls.md section 8).
    if (saved.scroll) { n.scrollTop = saved.scroll; }
}
"#;

pub(crate) const SLEEP_ADAPTER_JS: &str = "\
// Platform adapter: async sleep (Tier 7).
// `sleep(ms)` returns a Promise that resolves to
// `{__cui_tag: \"Ok\", value: undefined}` after `ms` milliseconds.
// The `await(...)` expression in cancho-ui lowers to `await expr`
// in JS, so the resolved value flows into the surrounding
// statement. Tier 7 does not implement cancellation yet - the
// `Cancelled` variant is matched but never produced.
function __cui_sleep_ms(ms) {
    return new Promise(function (resolve) {
        setTimeout(function () { resolve({__cui_tag: \"Ok\", value: undefined}); }, ms);
    });
}
";

pub(crate) const NETWORK_ADAPTER_JS: &str = "\
// Platform adapter: Network capability (Tier 9).
// `net.search(q)` returns a Promise that resolves to
// `{__cui_tag: \"Ok\", value: [...]}` after a simulated network
// delay. Tier 9 stub: the search filters a fixed corpus by
// substring, so the demo is fully interactive offline. The delay
// grows with the query length so slow requests demonstrably
// return AFTER fast ones - which is exactly what Tier 10
// cancellation protects against. A failing deadline resolves
// with `{__cui_tag: \"Err\", reason}` instead.
const __CUI_CORPUS = [
    \"Ada Lovelace\", \"Grace Hopper\", \"Linus Torvalds\",
    \"Andrey Kolmogorov\", \"Bertrand Russell\", \"Ada Marrero\",
];
function __cui_net_search(q) {
    return new Promise(function (resolve) {
        setTimeout(function () {
            const needle = String(q).trim().toLowerCase();
            const value = __CUI_CORPUS.filter(function (item) {
                return item.toLowerCase().indexOf(needle) !== -1;
            });
            resolve({__cui_tag: \"Ok\", value: value});
        }, 60 + 45 * String(q).length);
    });
}
";

/// The `net.get` adapter (docs/adapters.md section 9): a same-origin GET with a size bound and a deadline.
pub(crate) const NET_GET_ADAPTER_JS: &str = "\
// Platform adapter: Network capability, `net.get(url)` (docs/adapters.md section 9) and `net.fetch(url)` (section 15).
// `net.fetch` answers Ok for any HTTP status, with the JSON text {\"status\":N,\"body\":\"...\"}; `net.get` answers Err(\"HTTP N\")
// for a status outside 200 to 299.
// A GET of this page's own origin, or of a host the component's `requires Network(\"host\")` names: any other
// URL is answered with Err before a request is made, a redirect is refused (it could leave the allowed set), a
// body over 1 MiB or a reply later than 10 s is an Err, and every failure is a value, never a throw.
function __cui_net_get(url, hosts, cred) {
    return __cui_net_request(url, hosts, false, cred);
}
function __cui_net_fetch(url, hosts, cred) {
    return __cui_net_request(url, hosts, true, cred);
}
function __cui_net_request(url, hosts, envelope, cred) {
    var owner = typeof __cui_owner !== \"undefined\" ? __cui_owner : null;
    var err = function (reason) { return {__cui_tag: \"Err\", reason: reason}; };
    return new Promise(function (resolve) {
        var auth = __cui_credential_check(cred, resolve);
        if (auth === false) { return; }
        var target;
        try {
            target = new URL(String(url), window.location.href);
        } catch (e) {
            resolve(err(\"blocked: not a URL\"));
            return;
        }
        if (target.protocol !== \"http:\" && target.protocol !== \"https:\") {
            resolve(err(\"blocked: not an http(s) URL\"));
            return;
        }
        if (target.origin !== window.location.origin && hosts.indexOf(target.host.toLowerCase()) === -1) {
            resolve(err(\"blocked: \" + target.host + \" is neither this site nor an allowed host\"));
            return;
        }
        var timer = null;
        var stopped = false;
        var controller = typeof AbortController === \"function\" ? new AbortController() : null;
        if (controller) { timer = setTimeout(function () { controller.abort(); }, 10000); }
        if (owner && controller) { owner.abort = function () { stopped = true; controller.abort(); }; }
        var init = { method: \"GET\", credentials: \"same-origin\", redirect: \"manual\" };
        if (auth !== null && __cui_credential_applies(cred, target)) { init.headers = { Authorization: auth }; }
        if (controller) { init.signal = controller.signal; }
        fetch(target.href, init).then(function (response) {
            if (response.type === \"opaqueredirect\") { return err(\"blocked: redirect\"); }
            if (!response.ok && !envelope) { return err(\"HTTP \" + response.status); }
            return response.text().then(function (text) {
                if (text.length > 1048576) { return err(\"too large\"); }
                return {__cui_tag: \"Ok\", value: envelope ? JSON.stringify({status: response.status, body: text}) : text};
            });
        }).catch(function (e) {
            return err(e && e.name === \"AbortError\" ? (stopped ? \"aborted\" : \"timeout\") : \"network error\");
        }).then(function (result) {
            if (timer !== null) { clearTimeout(timer); }
            resolve(result);
        });
    });
}
";

/// The credential helpers (docs/adapters.md section 18), emitted with the request adapters when the module declares one.
pub(crate) const CREDENTIAL_ADAPTER_JS: &str = "\
// Credential capability (docs/adapters.md section 18). `cred` is {name, hosts, token} for a component that declared
// `requires Credential(name, hosts..)`, or null. A declared credential that was not supplied at mount, or that is not
// a plain visible-ASCII token, answers Err before any request is made; otherwise the value is \"Bearer <token>\".
// The header is attached only when the target is this page's own origin or a host the declaration names.
function __cui_credential_check(cred, resolve) {
    if (!cred) { return null; }
    var token = cred.token;
    if (typeof token !== \"string\" || token === \"\") {
        resolve({__cui_tag: \"Err\", reason: \"blocked: no credential \" + cred.name});
        return false;
    }
    if (!/^[\\x21-\\x7e]{1,4096}$/.test(token)) {
        resolve({__cui_tag: \"Err\", reason: \"blocked: credential \" + cred.name + \" is not a plain token\"});
        return false;
    }
    return \"Bearer \" + token;
}
function __cui_credential_applies(cred, target) {
    return !!cred && (target.origin === window.location.origin || cred.hosts.indexOf(target.host.toLowerCase()) !== -1);
}
";

/// The `net.post` adapter (docs/adapters.md section 17): a same-origin POST of a JSON text, answered like `net.fetch`.
pub(crate) const NET_POST_ADAPTER_JS: &str = "\
// Platform adapter: NetworkWrite capability, `net.post(url, body)` (docs/adapters.md section 17).
// A POST to this page's own origin and no other, with `body` as the request body (Content-Type: application/json) and
// the cookies of this origin. Answered like `net.fetch`: Ok with {\"status\":N,\"body\":\"...\"} for any HTTP status, Err for
// what produced no answer. A redirect is refused, a reply over 1 MiB is `too large`, a reply later than 10 s is `timeout`.
// It is not aborted when superseded or disposed: a write cut off half way is in an unknown state (section 17.3).
function __cui_net_post(url, body, cred) {
    var err = function (reason) { return {__cui_tag: \"Err\", reason: reason}; };
    return new Promise(function (resolve) {
        var auth = __cui_credential_check(cred, resolve);
        if (auth === false) { return; }
        var target;
        try {
            target = new URL(String(url), window.location.href);
        } catch (e) {
            resolve(err(\"blocked: not a URL\"));
            return;
        }
        if (target.protocol !== \"http:\" && target.protocol !== \"https:\") {
            resolve(err(\"blocked: not an http(s) URL\"));
            return;
        }
        if (target.origin !== window.location.origin) {
            resolve(err(\"blocked: a write reaches only this site's own origin\"));
            return;
        }
        var timer = null;
        var controller = typeof AbortController === \"function\" ? new AbortController() : null;
        if (controller) { timer = setTimeout(function () { controller.abort(); }, 10000); }
        var init = { method: \"POST\", credentials: \"same-origin\", redirect: \"manual\", headers: { \"Content-Type\": \"application/json\" }, body: String(body) };
        if (auth !== null) { init.headers.Authorization = auth; }
        if (controller) { init.signal = controller.signal; }
        fetch(target.href, init).then(function (response) {
            if (response.type === \"opaqueredirect\") { return err(\"blocked: redirect\"); }
            return response.text().then(function (text) {
                if (text.length > 1048576) { return err(\"too large\"); }
                return {__cui_tag: \"Ok\", value: JSON.stringify({status: response.status, body: text})};
            });
        }).catch(function (e) {
            return err(e && e.name === \"AbortError\" ? \"timeout\" : \"network error\");
        }).then(function (result) {
            if (timer !== null) { clearTimeout(timer); }
            resolve(result);
        });
    });
}
";

/// The `net.put` adapter (docs/adapters.md section 21): a same-origin PUT of a JSON text, answered like `net.post`.
pub(crate) const NET_PUT_ADAPTER_JS: &str = "\
// Platform adapter: NetworkWrite capability, `net.put(url, body)` (docs/adapters.md section 21).
// A PUT to this page's own origin and no other, with `body` as the request body (Content-Type: application/json) and
// the cookies of this origin. Answered like `net.fetch`: Ok with {\"status\":N,\"body\":\"...\"} for any HTTP status, Err for
// what produced no answer. A redirect is refused, a reply over 1 MiB is `too large`, a reply later than 10 s is `timeout`.
// It is not aborted when superseded or disposed: a write cut off half way is in an unknown state (section 17.1).
function __cui_net_put(url, body, cred) {
    var err = function (reason) { return {__cui_tag: \"Err\", reason: reason}; };
    return new Promise(function (resolve) {
        var auth = __cui_credential_check(cred, resolve);
        if (auth === false) { return; }
        var target;
        try {
            target = new URL(String(url), window.location.href);
        } catch (e) {
            resolve(err(\"blocked: not a URL\"));
            return;
        }
        if (target.protocol !== \"http:\" && target.protocol !== \"https:\") {
            resolve(err(\"blocked: not an http(s) URL\"));
            return;
        }
        if (target.origin !== window.location.origin) {
            resolve(err(\"blocked: a write reaches only this site's own origin\"));
            return;
        }
        var timer = null;
        var controller = typeof AbortController === \"function\" ? new AbortController() : null;
        if (controller) { timer = setTimeout(function () { controller.abort(); }, 10000); }
        var init = { method: \"PUT\", credentials: \"same-origin\", redirect: \"manual\", headers: { \"Content-Type\": \"application/json\" }, body: String(body) };
        if (auth !== null) { init.headers.Authorization = auth; }
        if (controller) { init.signal = controller.signal; }
        fetch(target.href, init).then(function (response) {
            if (response.type === \"opaqueredirect\") { return err(\"blocked: redirect\"); }
            return response.text().then(function (text) {
                if (text.length > 1048576) { return err(\"too large\"); }
                return {__cui_tag: \"Ok\", value: JSON.stringify({status: response.status, body: text})};
            });
        }).catch(function (e) {
            return err(e && e.name === \"AbortError\" ? \"timeout\" : \"network error\");
        }).then(function (result) {
            if (timer !== null) { clearTimeout(timer); }
            resolve(result);
        });
    });
}
";

/// The `net.delete` adapter (docs/adapters.md section 21): a same-origin DELETE with no body, answered like `net.post`.
pub(crate) const NET_DELETE_ADAPTER_JS: &str = "\
// Platform adapter: NetworkWrite capability, `net.delete(url)` (docs/adapters.md section 21).
// A DELETE to this page's own origin and no other, with no body and no Content-Type, and
// the cookies of this origin. Answered like `net.fetch`: Ok with {\"status\":N,\"body\":\"...\"} for any HTTP status, Err for
// what produced no answer. A redirect is refused, a reply over 1 MiB is `too large`, a reply later than 10 s is `timeout`.
// It is not aborted when superseded or disposed: a write cut off half way is in an unknown state (section 17.1).
function __cui_net_delete(url, cred) {
    var err = function (reason) { return {__cui_tag: \"Err\", reason: reason}; };
    return new Promise(function (resolve) {
        var auth = __cui_credential_check(cred, resolve);
        if (auth === false) { return; }
        var target;
        try {
            target = new URL(String(url), window.location.href);
        } catch (e) {
            resolve(err(\"blocked: not a URL\"));
            return;
        }
        if (target.protocol !== \"http:\" && target.protocol !== \"https:\") {
            resolve(err(\"blocked: not an http(s) URL\"));
            return;
        }
        if (target.origin !== window.location.origin) {
            resolve(err(\"blocked: a write reaches only this site's own origin\"));
            return;
        }
        var timer = null;
        var controller = typeof AbortController === \"function\" ? new AbortController() : null;
        if (controller) { timer = setTimeout(function () { controller.abort(); }, 10000); }
        var init = { method: \"DELETE\", credentials: \"same-origin\", redirect: \"manual\", headers: {} };
        if (auth !== null) { init.headers.Authorization = auth; }
        if (controller) { init.signal = controller.signal; }
        fetch(target.href, init).then(function (response) {
            if (response.type === \"opaqueredirect\") { return err(\"blocked: redirect\"); }
            return response.text().then(function (text) {
                if (text.length > 1048576) { return err(\"too large\"); }
                return {__cui_tag: \"Ok\", value: JSON.stringify({status: response.status, body: text})};
            });
        }).catch(function (e) {
            return err(e && e.name === \"AbortError\" ? \"timeout\" : \"network error\");
        }).then(function (result) {
            if (timer !== null) { clearTimeout(timer); }
            resolve(result);
        });
    });
}
";

/// The `location.current` adapter (docs/adapters.md section 11): the browser's Geolocation, rounded in the adapter.
pub(crate) const LOCATION_ADAPTER_JS: &str = "\
// Platform adapter: Location capability, `location.current()` (docs/adapters.md section 11).
// Resolves to \"lat,lon\" in decimal degrees, rounded to `digits` decimals (2, about 1 km, unless the component
// asked for \"fine\": 5). The browser asks the user; a refusal, a timeout or a missing API is an Err value.
function __cui_location_current(digits) {
    var err = function (reason) { return {__cui_tag: \"Err\", reason: reason}; };
    return new Promise(function (resolve) {
        if (typeof navigator === \"undefined\" || !navigator.geolocation) {
            resolve(err(\"unavailable\"));
            return;
        }
        try {
            navigator.geolocation.getCurrentPosition(function (pos) {
                resolve({__cui_tag: \"Ok\", value: pos.coords.latitude.toFixed(digits) + \",\" + pos.coords.longitude.toFixed(digits)});
            }, function (e) {
                resolve(err(e && e.code === 1 ? \"permission denied\" : e && e.code === 3 ? \"timeout\" : \"position unavailable\"));
            }, { enableHighAccuracy: digits > 2, timeout: 10000, maximumAge: 60000 });
        } catch (e) {
            resolve(err(\"unavailable\"));
        }
    });
}
";

pub(crate) const CANCEL_ADAPTER_JS: &str = "\
// Platform adapter: runtime cancellation (Tier 10), with per-
// component disposal (Tier 17).
// `await(expr)` lowers to `__cui_call(group, start, inflight)`:
//   - the inflight table is *per component instance*: two
//     components awaiting the same effect never cancel each
//     other's requests;
//   - within one component, starting a call cancels the group's
//     still-pending request (stale-result protection: a newer
//     search supersedes an older one) - a superseded request
//     resolving later throws the `__cui_cancelled` sentinel and
//     the handler's try/catch swallows it: the rest of that
//     handler is *dropped* (drop at the await);
//   - component disposal (a slot removal, or the parent's
//     disposal) marks the pending token `disposed`, and the
//     pending await *resolves* to `{__cui_tag:\"Cancelled\"}` -
//     the `match`'s `Cancelled` arm dispatches. The arm is
//     compile-time mandatory for an awaited field, so a disposed
//     component's handler always has the program's cancellation
//     design to run; it then continues on a detached subtree.
const __cui_cancelled = Symbol(\"__cui_cancelled\");
//   - a request that can be stopped says how: `__cui_net_request` reads `__cui_owner` as it starts and sets
//     `token.abort`, and a superseded or disposed request is aborted, so the browser stops the underlying fetch
//     (docs/adapters.md 9.5) instead of letting it complete to a result nobody reads.
let __cui_owner = null;
function __cui_call(group, start, inflight) {
    const prev = inflight[group];
    if (prev) { prev.cancelled = true; if (prev.abort) { prev.abort(); } }
    const token = { cancelled: false, disposed: false, abort: null };
    inflight[group] = token;
    __cui_owner = token;
    const started = start();
    __cui_owner = null;
    return started.then(function (res) {
        if (token.cancelled) { throw __cui_cancelled; }
        if (token.disposed) { return {__cui_tag: \"Cancelled\"}; }
        if (inflight[group] === token) { inflight[group] = null; }
        return res;
    });
}
";
