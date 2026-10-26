# Diagnostic format

> **Status: Phase 0.** This document locks the shape of every
> diagnostic and report item in cancho-ui. The first refusal the
> tool emits against a real input pins the schema; the schema
> does not drift between releases.

---

## 1. Position

Every refusal is **a structured value**, not a string. A
diagnostic has fields an agent reads; a human-readable
sentence is parallel.

The shape inherits from Cancho Core's
[`agent-errors.md`](https://github.com/alpibrusl/cancho/blob/main/docs/agent-errors.md):
58 named rule tags, each with a stable code, a span, and a
message. cancho-ui extends the shape with the **guarantee tier**
field, which the security model
([`docs/security-model.md`](../../docs/security-model.md))
introduces.

---

## 2. The diagnostic shape

```text
Diagnostic {
    code: String,                       // stable rule tag, kebab-case
    severity: Severity,                 // Error | Warning
    tier: Tier,                         // Static | Tested | Runtime | Trust | Unknown
    span: Option<Span>,
    subject: Subject,
    message: String,                    // short explanation an agent can read
    expected: Option<Value>,            // when relevant
    actual: Option<Value>,              // when relevant
    remediation: Option<Remediation>,
    related: Vec<Related>,              // cross-references
    notes: Vec<String>,                 // anything else, opt-in
}

Severity = Error | Warning

Tier = Static | Tested | Runtime | Trust | Unknown

Subject =
    | Component { name: String }
    | State { component: String, field: String }
    | Event { name: String }
    | Effect { name: String }
    | Capability { name: String }
    | Lifecycle { component: String, phase: String }
    | Foreign { scope: String, symbol: String }
    | Target { name: String }
    | Accessibility { component: String, role: String }
    | Ir { path: String }

Value =
    | Type(...)
    | Expr(...)
    | String(String)
    | Number(Number)
    | Bool(bool)
    | Null

Remediation {
    description: String,                // one sentence
    suggestion: Option<PatchHint>,       // a structured patch when the compiler can justify one
    confidence: Confidence,             // High | Medium | Low
}

Confidence = High | Medium | Low

Related {
    code: String,
    span: Option<Span>,
    message: String,
}

Span {
    file: String,
    start: Position,                    // 1-indexed line, 1-indexed column
    end: Position,
}
```

### 2.1 Required fields

Every diagnostic carries `code`, `severity`, `tier`, `subject`,
`message`. The rest are present only when the diagnostic is
about them — `span` is `None` for diagnostics that do not come
from a source-located check (e.g. an internal compiler error).

### 2.2 No optional `message`

`message` is always present and always a non-empty string. The
message is what an agent reads first; it is the entry point.

### 2.3 No LLM-generated explanations

The `remediation.suggestion` field, when present, is a
**structured patch hint** produced by the compiler's own
analysis, not by an LLM. The agent may apply the hint; the
compiler re-checks the result. The shape is the same one
Cancho Core uses for its rule-tagged diagnostics.

---

## 3. The rule catalog

The full Phase 1 rule catalog. Each entry's `code` is the
**stable rule tag** used by the diagnostic, by the refusal test
in `examples/refusal/`, and by the evidence report.

| `code` | `subject` | `severity` | `tier` | Meaning |
|---|---|---|---|---|
| `state-unused` | `State` | `Error` | `Static` | A `state` declared but never read or updated |
| `state-after-dispose` | `State` | `Error` | `Static + Runtime` | A transition updating state in a disposed component |
| `state-escapes` | `State` | `Error` | `Static` | A component returning its owned state as an owned value |
| `effect-not-declared` | `Effect` | `Error` | `Static` | A transition body performing an effect the component has not declared |
| `state-not-owned` | `State` | `Error` | `Static` | A transition updating a state field not declared on the component |
| `test-asserts-unspecified` | `Effect` | `Error` | `Tested` | A test asserting an ordering the language does not guarantee |
| `match-non-exhaustive` | `Effect` | `Error` | `Static` | A `match` on an effect result missing an `Err` or `Cancelled` arm |
| `call-unknown` | `Component` | `Error` | `Static` | A call to a function the language does not have (`sqrt(x)`, `pow(a, b)`, a misspelt builtin; `max`, `min`, `abs`, `floor`, `ceil` and `round` exist since issue #43). It used to pass `check` and be emitted as `x = /* unsupported call */;`, a JavaScript syntax error that stopped the whole page. *(Built 2026-10-21, found building the ops console.)* |
| `pattern-literal-unsupported` | `Component` | `Error` | `Static` | A `match` arm whose pattern is `true` or `false`: a bare name is a binding that matches everything, so the arm would always win (there are no literal patterns). *(Built 2026-10-21, issue #26.)* |
| `binding-unknown` | `Component` | `Error` | `Static` | A key that is not a binding of the built-in element it is on (`title:` on a `Button`, `disabled:` on a `Column`). It used to be accepted and dropped or written as a stray attribute. *(Built 2026-10-21, issue #35, `docs/controls.md`.)* |
| `disabled-not-bool` | `Component` | `Error` | `Static` | A `disabled:` that is not `true`, `false` or the name of a `Bool` state field of the component. *(Built 2026-10-21, issue #35.)* |
| `submit-event-invalid` | `Component` | `Error` | `Static` | A `submit:` on a `TextField` naming an event the component does not declare, or one that takes parameters (Enter gives none). *(Built 2026-10-21, issue #36, `docs/controls.md` section 3.)* |
| `event-unknown` | `Component` | `Error` | `Static` | An `on:` naming an event the component does not declare (with or without arguments). It used to compile and kill the page on its first render. *(Built 2026-10-21, issue #41, `docs/row-actions.md`.)* |
| `event-arity-mismatch` | `Component` | `Error` | `Static` | An `on:` whose arguments are not the parameters of the handler: the wrong number of arguments, a bare `on:` on a button for a handler with parameters (a click has none; it used to receive the `MouseEvent`), or a field for a handler with two. *(Built 2026-10-21, issue #41.)* |
| `event-args-unsupported` | `Component` | `Error` | `Static` | `on: Event(..)` with arguments on anything but a `Button`. *(Built 2026-10-21, issue #41.)* |
| `if-else-required` | `Component` | `Error` | `Static` | An `if` expression without `else` (`out = if c { 1 };`): an `if` has a value on both sides. *(Built 2026-10-22, issue #40, `docs/conditionals.md`.)* |
| `if-effect-in-expression` | `Component` | `Error` | `Static` | An effect call (`net.*`, `storage.*`, `clock.*`, `location.*`), `sleep` or `await(..)` in the condition or a branch of an `if`: the effects a handler issues would depend on a value the replay trace has to supply. *(Built 2026-10-22, issue #40.)* The condition of an `if` *statement* may not either (`docs/handler-sequences.md`, #93); the branches of a statement may. |
| `condition-not-bool` | `Component` | `Error` | `Static` | The condition of an `if`, or an operand of `&&`, `\|\|`, `!`, that is known not to be a `Bool` (a number, a string, a list, a `Result`/`Option`). A type the pass cannot see (an event parameter, a `For` binding) is not refused here; replay refuses it when it runs (`replay-type-error`). *(Built 2026-10-22, issue #40.)* Also the `when:` of a view `If { when: .. }` (`docs/view-conditional.md`, issue #90). Also the condition of an `if` statement. |
| `operand-type-mismatch` | `Component` | `Error` | `Static` | An operand of `+` that is known to be a list, a `Result`, an `Option` or a `Request`, or a `Bool` added to a number or a `Bool`. Two lists are joined with `concat(a, b)`. A type the pass cannot see is not refused here; replay refuses it when it runs (`replay-type-error`). *(Built 2026-10-22, issue #39.)* |
| `if-branch-type-mismatch` | `Component` | `Error` | `Static` | The two branches of an `if` have different known types (a number and a string, a Bool and a list). *(Built 2026-10-22, issue #40.)* |
| `comparison-type-mismatch` | `Component` | `Error` | `Static` | A `==`, `!=`, `<`, `<=`, `>` or `>=` whose operand types are both known and are not a pair replay defines: `==`/`!=` need two numbers, two strings or two bools (a number against a string is always false, a list or a `Result` is a replay fault and a reference comparison in the page); the orderings need two numbers or two strings. A type the pass cannot see (an event parameter, a `For` binding) is not refused. *(Built 2026-10-23, issue #95, `docs/comparisons.md`.)* |
| `name-unknown` | `Component` | `Error` | `Static` | A name in a handler, a view expression, a label, an event argument or an `over:` that is neither a state field nor a binding in scope (an event parameter, a `For` binding, a lambda parameter, a `match` binding). It used to pass `check` and blank the page with a `ReferenceError`; the message lists what is in scope. *(Built 2026-10-22, issue #55.)* Also in the `when:` of an `If`. Also in a `block`, where a handler's parameters are not in scope. |
| `tone-invalid` | `Component` | `Error` | `Static` | A `tone:` on a `Text` that is not `"ok"`, `"warn"`, `"error"` or the name of a `String` state field. *(Built 2026-10-21, issue #42, `docs/presentation.md` section 3.)* |
| `layout-value-invalid` | `Component` | `Error` | `Static` | A `gap:` outside 0 to 4 or a `max_width:` outside 10 to 200 on a `Column` or `Row` (the `gap:` also on a `Grid`). *(Built 2026-10-21, issue #42, section 2.)* |
| `block-unknown` | `Component` | `Error` | `Static` | A `run Name;` of a block the component does not declare. *(Built 2026-10-22, issue #93, `docs/handler-sequences.md`.)* |
| `block-duplicate` | `Component` | `Error` | `Static` | Two `block`s of one name in a component. *(Built 2026-10-22, issue #93.)* |
| `block-name-conflict` | `Component` | `Error` | `Static` | A `block` named like a state field or an event of the component. *(Built 2026-10-22, issue #93.)* |
| `block-cycle` | `Component` | `Error` | `Static` | A `block` that runs itself, directly or through other blocks: its expansion would never end. *(Built 2026-10-22, issue #93.)* |
| `block-expansion-too-deep` | `Component` | `Error` | `Static` | Blocks that run blocks, and the `if`s inside what they run, nest more than 32 deep once the `run`s are replaced (a chain of 33 blocks that each run the next). *(Issue #99, `docs/nesting.md` section 5.)* |
| `block-expansion-too-large` | `Component` | `Error` | `Static` | A handler or block that has more than 10,000 statements once its `run`s are replaced (blocks that run blocks several times multiply). *(Built 2026-10-22, issue #93.)* |
| `input-type-invalid` | `Component` | `Error` | `Static` | A `type:` on a `TextField` that is not one of `text`, `password`, `email`, `search`, `tel`, `url`, `number` written as a string literal (a state field, `checkbox`, `PASSWORD`; `number` was refused until issue #111, `docs/number-input.md`). `type:` on anything but a `TextField` is `binding-unknown`. *(Built 2026-10-22, issue #76, `docs/controls.md` section 5.)* |
| `input-attribute-invalid` | `Component` | `Error` | `Static` | An `autocomplete:` that is not `on`, `off` or one field name of the HTML autofill list; an `inputmode:` that is not one of none, text, decimal, numeric, tel, search, email, url; a `maxlength:` that is not an integer literal from 1 to 100000; a `name:` that is not 1 to 64 letters, digits, `-`, `_`, `.` starting with a letter; a `rows:` of a `TextArea` that is not an integer literal from 1 to 30. These are bindings of a `TextField` and a `TextArea` (`rows:` of a `TextArea` only); on anything else each is `binding-unknown`. *(Built 2026-10-22, issue #91, `docs/controls.md` section 6.)* |
| `error-binding-invalid` | `Component` | `Error` | `Static` | An `error:` on a `TextField` that is not the name of a `String` state field of the component (a literal, a number, an `Int` or `Bool` field, a name that is no field). On anything but a `TextField` it is `binding-unknown`. *(Built 2026-10-23, issue #76, `docs/controls.md` section 7.)* |
| `checked-binding-invalid` | `Component` | `Error` | `Static` | A `checked:` on a `Checkbox` that is not the name of a `Bool` state field of the component (a literal, a `String` or `Int` field, a name that is no field). On anything but a `Checkbox` it is `binding-unknown`. *(Built 2026-10-23, issue #76, `docs/controls.md` section 9.)* |
| `checkbox-incomplete` | `Component` | `Error` | `Static` | A `Checkbox` without `checked:` (the page would forget the box at the next redraw) or without `on:` (it would tell the program nothing). *(Built 2026-10-23, issue #76, `docs/controls.md` section 9.)* |
| `select-incomplete` | `Component` | `Error` | `Static` | A `Select` without `options:` (the list), `value:` (the `String` field holding the choice) or `on:` (the event). *(Built 2026-10-23, issue #76, `docs/controls.md` section 10.)* |
| `select-binding-invalid` | `Component` | `Error` | `Static` | An `options:` that is not a list of string literals or the name of a `List` state field, or a `value:` of a `Select` that is not the name of a `String` state field. `options:` on anything but a `Select` is `binding-unknown`. *(Built 2026-10-23, issue #76, `docs/controls.md` section 10.)* |
| `route-event-unknown` | `Component` | `Error` | `Static` | A `route Event;` naming an event the component does not handle. *(Built 2026-10-23, issue #75, `docs/routing.md` section 4.)* |
| `route-event-params` | `Component` | `Error` | `Static` | A `route Event;` whose event does not take exactly one parameter (the path). *(Built 2026-10-23, issue #75.)* |
| `route-duplicate` | `Component` | `Error` | `Static` | Two `route` lines in a component: one event receives every path. *(Built 2026-10-23, issue #75.)* |
| `navlink-incomplete` | `Component` | `Error` | `Static` | A `NavLink` without `label:` (its text) or without `to:` (the path). *(Built 2026-10-23, issue #75, `docs/routing.md` section 4.)* |
| `navlink-target-invalid` | `Component` | `Error` | `Static` | A literal `to:` of a `NavLink` that is not a path: it must start with `/`, hold no space, `#`, `"`, `<`, `>`, backtick or character outside printable ASCII, and be at most 2,000 characters. `to:` on anything but a `NavLink` is `binding-unknown`. *(Built 2026-10-23, issue #75.)* |
| `file-field-incomplete` | `Component` | `Error` | `Static` | A `FileField` without `label:` or without `on:`. *(Built 2026-10-23, issue #79, `docs/file-upload.md`.)* |
| `file-accept-invalid` | `Component` | `Error` | `Static` | An `accept:` of a `FileField` that is not a string literal of comma-separated tokens (`.ext`, `type/subtype`, `type/*`; no spaces; at most 20). `accept:` on anything but a `FileField` is `binding-unknown`. *(Built 2026-10-23, issue #79.)* |
| `subscribe-event-unknown` | `Component` | `Error` | `Static` | A `subscribe Event "url";` (or its `status Event`) naming an event the component does not handle. *(Built 2026-10-24, issue #78, `docs/streams.md` section 3.)* |
| `subscribe-event-params` | `Component` | `Error` | `Static` | A `subscribe` whose message event, or whose `status` event, does not take exactly one parameter (the message's text, the status word). *(Built 2026-10-24, issue #78.)* |
| `subscribe-url-invalid` | `Component` | `Error` | `Static` | A `subscribe` whose address is not a string literal naming a path (`/api/stream`, `api/stream`) or an `http(s)://` URL with a host: empty, over 2,000 characters, with a space, backslash or character outside printable ASCII, with a fragment (`#`), protocol-relative (`//host/x`), of another scheme (`ws:`, `ftp:`, `javascript:`, `data:`), or carrying credentials (`user@host`). *(Built 2026-10-24, issue #78.)* |
| `subscribe-duplicate` | `Component` | `Error` | `Static` | Two `subscribe` lines in a component: it listens to one stream. *(Built 2026-10-24, issue #78.)* |
| `message-language-unknown` | `Component` | `Error` | `Static` | A `messages` catalog for a language the compiler has no formats for (`en es de fr pt ru ar he ja hi`). *(Built 2026-10-24, issue #82, `docs/i18n.md` section 3.)* |
| `message-key-missing` | `Component` | `Error` | `Static` | A message one language has and another lacks (either way round): every language of a component has the same keys. *(Built 2026-10-24, issue #82.)* |
| `message-key-duplicate` | `Component` | `Error` | `Static` | The same key twice in one language, or two catalogs for one language. *(Built 2026-10-24, issue #82.)* |
| `message-hole-mismatch` | `Component` | `Error` | `Static` | A message whose `{name}` holes differ between languages, or that is a plural in one language and a plain text in another. *(Built 2026-10-24, issue #82.)* |
| `message-plural-incomplete` | `Component` | `Error` | `Static` | A plural message that lacks a form its language has, has one it does not, or gives one twice (English `one`, `other`; Spanish also `many`; Russian `one`, `few`, `many`, `other`; Arabic six; Japanese `other`). *(Built 2026-10-24, issue #82.)* |
| `message-text-invalid` | `Component` | `Error` | `Static` | A message text with a brace that is not a hole (`{name}`: letters, digits and `_`, not starting with a digit), or a plural form with a hole other than its count. *(Built 2026-10-24, issue #82.)* |
| `message-key-unknown` | `Component` | `Error` | `Static` | A `t("key")` whose key is in no catalog of the component. *(Built 2026-10-24, issue #82.)* |
| `message-call-invalid` | `Component` | `Error` | `Static` | A `t`, `fmt.*` or `locale.*` call of the wrong shape: a key that is not a string literal, a number of arguments the message does not take, a `fmt.date` style that is not one of the literals `"short"`, `"medium"`, `"long"`, `"full"`. *(Built 2026-10-24, issue #82.)* |
| `sortlist-incomplete` | `Component` | `Error` | `Static` | A `SortList` without `label:` (its name), `items:` (the `List` field it shows) or `on:` (the event that hears a move). *(Built 2026-10-24, issue #80, `docs/drag-and-drop.md` section 3.)* |
| `sortlist-binding-invalid` | `Component` | `Error` | `Static` | An `items:` of a `SortList` that is not the name of a `List` state field of the component. `items:` on anything but a `SortList`, and any key a `SortList` does not take, is `binding-unknown`; an `on:` event that does not take two parameters is `event-arity-mismatch`. *(Built 2026-10-24, issue #80.)* |
| `map-incomplete` | `Component` | `Error` | `Static` | A `Map` without `label:`, `center:`, `places:`, `zoom:`, `tiles:` or `attribution:`. *(Issue #81, `docs/maps.md` section 10.)* |
| `map-tiles-invalid` | `Component` | `Error` | `Static` | A `Map`'s `tiles:` that is not a text literal tile address: without `{z}`, `{x}` and `{y}` once each, with another brace, a space, quote, `<`, `>`, backslash or `#`, with a user name or password, a scheme other than `https` (`http` for `localhost` and `127.0.0.1` only), a host that is not a name with an optional port, or neither an address nor a path on the page's own origin. |
| `map-host-undeclared` | `Component` | `Error` | `Static` | A `Map` whose tile host is not in the component's `requires Network("host")` (a bound is compared lower-cased, port included), or a `Map` in a component with no `Network` at all (a path on the page's own origin needs the bare capability). The tile host is authority and the semantic diff reports it. |
| `map-zoom-invalid` | `Component` | `Error` | `Static` | A `Map`'s `zoom:` that is not a whole number from 0 to 19, or from 0 to 13 in a component with a bare `requires Location` (or `Location("coarse")`): a tile finer than a position rounded to 1 km says more than the program knows. |
| `map-binding-invalid` | `Component` | `Error` | `Static` | A `Map`'s `center:` that is not a `String` state field or a text, `places:` that is not a `List` state field, or `attribution:` that is empty or longer than 200 characters. |
| `sortlist-in-loop` | `Component` | `Error` | `Static` | A `SortList` inside a `For`: one list per place in the source. *(Built 2026-10-24, issue #80.)* |
| `file-limit-invalid` | `Component` | `Error` | `Static` | A `max_bytes:` of a `FileField` that is not an integer literal from 1 to 1,048,576. On anything else it is `binding-unknown`. *(Built 2026-10-23, issue #79.)* |
| `file-field-in-loop` | `Component` | `Error` | `Static` | A `FileField` drawn inside a `For`: the page keeps the chosen file by the place of the field in the source, and every row would share one. *(Built 2026-10-23, issue #79.)* |
| `chart-incomplete` | `Component` | `Error` | `Static` | A `Chart` without `label:`, `kind:` or `values:`. *(Built 2026-10-24, issue #77, `docs/charts.md`.)* |
| `chart-kind-invalid` | `Component` | `Error` | `Static` | A `kind:` that is not the string literal `"line"` or `"bar"`. *(Built 2026-10-24, issue #77.)* |
| `chart-values-invalid` | `Component` | `Error` | `Static` | A `values:` that is not the name of a `List[Int]` or `List[Float]` state field, or a list literal of number literals. *(Built 2026-10-24, issue #77.)* |
| `chart-labels-invalid` | `Component` | `Error` | `Static` | A `labels:` that is not the name of a `List[String]` state field, or a list literal of string literals. *(Built 2026-10-24, issue #77.)* |
| `chart-unit-invalid` | `Component` | `Error` | `Static` | A `unit:` that is not a string literal of at most 12 characters. *(Built 2026-10-24, issue #77.)* |
| `chart-in-loop` | `Component` | `Error` | `Static` | A `Chart` drawn inside a `For`. *(Built 2026-10-24, issue #77.)* The keys of a chart on another element (and any other key on a `Chart`) are `binding-unknown`. |
| `theme-token-unknown` | `Component` | `Error` | `Static` | In a `theme { .. }` (docs/theme.md): a scheme other than `light` and `dark`, a colour token that is not one of the eighteen (bg, fg, muted, line, surface, soft, ink, accent, accent_fg, accent_soft, good, warn, warn_bg, warn_line, bad, bad_bg, code_bg, code_fg), a font role other than `body`, `mono` and `hand`, a font key other than family, file, sha256, licence and weight, or any of these written twice. *(Built 2026-10-24.)* A token written with a dash (`accent-fg`) is a `parse-unexpected-token`: the lexer reads a minus. |
| `theme-colour-invalid` | `Component` | `Error` | `Static` | A token's value that is not a string of six hex digits behind a `#` (`"#rrggbb"`): a name, three digits, alpha, a function, an empty string, anything else. *(Built 2026-10-24.)* |
| `theme-scheme-missing` | `Component` | `Error` | `Static` | A theme without its `light` or without its `dark` scheme, or a scheme that lacks one of the eighteen tokens (the message names them). There is no default: the compiler writes the dark mode from the `dark` block. *(Built 2026-10-24.)* |
| `theme-contrast` | `Component` | `Error` | `Static` | A pair of the 14 checked pairs of a scheme (fg on bg, fg on surface, muted on bg, surface and soft, accent on bg, surface and accent_soft, accent_fg on accent, good on bg, warn on warn_bg, bad on bad_bg and on bg, code_fg on code_bg) under 4.5:1 (WCAG 2.x 1.4.3). The message names every failing pair as `scheme: text on background is R:1`, with R cut (not rounded) to two decimals. *(Built 2026-10-24.)* |
| `theme-duplicate` | `Component` | `Error` | `Static` | More than one `theme { .. }` in a file, or one in the entry file and one in an imported module. *(Built 2026-10-24.)* |
| `font-remote` | `Component` | `Error` | `Static` | A font whose `file:` is an address (`http`, `https`, `//host`, `data:`, anything with `://`). A font is a file of the program, declared with its SHA-256, and copied into the build: the page makes no request to another host. *(Built 2026-10-24.)* |
| `font-licence-missing` | `Component` | `Error` | `Static` | A font with no `licence:`, or one that is not an identifier of letters, digits, `.`, `+`, `-` (1 to 40 characters). *(Built 2026-10-24.)* |
| `font-hash-mismatch` | `Component` | `Error` | `Static` | A font's `sha256:` that is not 64 hex digits, or is not the SHA-256 of its file (the message gives the hash the file has). Checked by `check` as well as `build`: it reads the file. *(Built 2026-10-24.)* |
| `font-file-missing` | `Component` | `Error` | `Static` | A font's file that cannot be read (relative to the entry file's directory). *(Built 2026-10-24; not in the list the user gave: the requirement that the compiler checks the file exists needed a tag.)* |
| `font-incomplete` | `Component` | `Error` | `Static` | A font without `family`, `file` or `sha256`, or with a family of more than 64 characters or characters other than letters, digits, space, `-`, `_`; a file that is not a relative path of letters, digits, `.`, `_`, `-`, `/` ending in `.woff2` without `..` segments; a `weight` that is not one number or two from 1 to 1000. *(Built 2026-10-24; added for the same reason as `font-file-missing`.)* |
| `heading-level-invalid` | `Component` | `Error` | `Static` | A `Heading` with no `level:`, or one that is not an integer literal from 1 to 4. *(Built 2026-10-24, docs/structure.md.)* |
| `heading-level-skipped` | `Component` | `Error` | `Static` | A heading whose level is more than one deeper than the heading before it in the same component's view (an `h3` straight after an `h1`): the outline would have a hole. An instance of another component forgets the order. *(Built 2026-10-24.)* |
| `heading-empty` | `Component` | `Error` | `Static` | A `Heading` with no text, or only spaces. *(Built 2026-10-24.)* |
| `id-invalid` | `Component` | `Error` | `Static` | An `id:` that is not a string literal of a letter then letters, digits, `-`, `_` (at most 64), or that starts with `cui-`, the compiler's own prefix. *(Built 2026-10-24.)* |
| `anchor-duplicate` | `Component` | `Error` | `Static` | Two elements of a program with the same `id:`, or an `id:` inside a `For`, which repeats it for every item. *(Built 2026-10-24.)* |
| `anchor-unknown` | `Component` | `Error` | `Static` | A `Link` to `#x` when no element of the program has `id: "x"`. *(Built 2026-10-24.)* A `#/path` is a route: `NavLink`. |
| `link-scheme-refused` | `Component` | `Error` | `Static` | A `Link` whose `to:` is not an `https:`/`http:` address with a host and no user name, a `mailto:` with an address, a `#id`, or a relative path: any other scheme (`javascript:`, `data:`, `vbscript:`, `file:`, `ftp:`), a `//host` that hides its scheme, a space or control character anywhere in it (a browser drops tabs inside a scheme), an empty `to:`, a bare `#`. *(Built 2026-10-24.)* |
| `link-to-invalid` | `Component` | `Error` | `Static` | A `Link` with no `to:`, or one that is not a string literal. *(Built 2026-10-24; a link built from data comes later.)* |
| `link-label-missing` | `Component` | `Error` | `Static` | A `Link` with no text. *(Built 2026-10-24.)* |
| `link-label-generic` | `Component` | `Error` | `Static` | A `Link` whose literal text is "here", "click here", "click", "link", "this link", "this", "more", "read more" or "learn more" (case and punctuation ignored): it does not say where the link goes. *(Built 2026-10-24.)* |
| `link-appearance-invalid` | `Component` | `Error` | `Static` | A link `appearance:` that is not `"primary"`, `"secondary"` or `"quiet"`. *(Built 2026-10-24.)* |
| `text-style-invalid` | `Component` | `Error` | `Static` | A `Text` `style:` that is not `"eyebrow"`, `"lead"`, `"note"` or `"label"`. *(Built 2026-10-24.)* |
| `details-incomplete` | `Component` | `Error` | `Static` | A `Details` with no `summary:` and no `Summary` child, or an empty `summary:`, or both, or a `Summary` that is not its first child, or a second one; a `Summary` outside a `Details`. *(Built 2026-10-24.)* |
| `caption-misplaced` | `Component` | `Error` | `Static` | A `Caption` that is not a direct child of a `Figure`, or a `Figure` with two. *(Built 2026-10-24.)* |
| `landmark-duplicate` | `Component` | `Error` | `Static` | A second `Main` in a program. *(Built 2026-10-24.)* |
| `nav-label-missing` | `Component` | `Error` | `Static` | A program with more than one `Nav` where one has no `label:`. *(Built 2026-10-24.)* |
| `rich-child-invalid` | `Component` | `Error` | `Static` | In inline content (a `Rich`, a `Heading`, a `Caption`, a `Summary`, a `Strong`, an `Em`, a `Small`, a `Link`): an element that is not `Strong`, `Em`, `Small`, `Break`, `Code` or `Link` (a `Text`, a `Button`, a `Column`, a `Rich`); a `Link` inside a `Link`; a `Break` with something in it. *(Built 2026-10-24, docs/structure.md section 7.)* |
| `rich-empty` | `Component` | `Error` | `Static` | A `Rich`, `Strong`, `Em` or `Small` with no content or only spaces. *(Built 2026-10-24.)* |
| `inline-outside-rich` | `Component` | `Error` | `Static` | A `Strong`, `Em`, `Small` or `Break` (or a `Code` that is `<code>`) anywhere but inside inline content. *(Built 2026-10-24.)* |
| `code-copy-invalid` | `Component` | `Error` | `Static` | A `Code` `copy:` that is not `true` or `false`. *(Built 2026-10-24, docs/structure.md section 8.)* |
| `code-max-height-invalid` | `Component` | `Error` | `Static` | A `Code` `max_height:` that is not an integer literal from 4 to 100 (rem). *(Built 2026-10-24.)* |
| `code-child-invalid` | `Component` | `Error` | `Static` | A `Code` (a box or the `<code>` of a sentence) that holds anything but text. *(Built 2026-10-24.)* |
| `clipboard-not-declared` | `Component` | `Error` | `Static` | A `Code { copy: true }` in a component that does not declare `requires Clipboard;`. A `requires Clipboard;` with no copy button is the existing `capability-unused`. *(Built 2026-10-24.)* |
| `image-src-invalid` | `Component` | `Error` | `Static` | An `Image` with no `src:`, or a `src:` or `fallback:` that is not a text literal naming a file of the site: an address of another host, an absolute path, a `..`, a character other than letters, digits and `. _ - /`, a format other than png, jpg, jpeg, webp, avif and gif (SVG can carry script); or a `fallback:` that is the `src:` or is in the same format. *(Built 2026-10-24, docs/structure.md section 10.)* |
| `image-alt-missing` | `Component` | `Error` | `Static` | An `Image` with neither `alt:` nor `decorative: true`; an `alt:` with nothing but spaces; a `decorative:` that is not `true`. *(Built 2026-10-24.)* |
| `image-alt-conflict` | `Component` | `Error` | `Static` | An `Image` with both `alt:` and `decorative: true`. *(Built 2026-10-24.)* |
| `image-size-missing` | `Component` | `Error` | `Static` | An `Image` without both `width:` and `height:`. *(Built 2026-10-24.)* |
| `image-size-invalid` | `Component` | `Error` | `Static` | A `width:` or `height:` that is not an integer literal from 1 to 10000. *(Built 2026-10-24.)* |
| `image-child-invalid` | `Component` | `Error` | `Static` | An `Image` with something in it. *(Built 2026-10-24.)* |
| `image-priority-invalid` | `Component` | `Error` | `Static` | A `priority:` that is not `true` or `false`. *(Built 2026-10-24.)* |
| `ratio-invalid` | `Component` | `Error` | `Static` | An `Image` or `Media` `ratio:` that is not one of "1:1", "4:3", "3:2", "16:9", "21:9", "3:4", "2:3". *(Built 2026-10-24.)* |
| `media-kind-invalid` | `Component` | `Error` | `Static` | A `Media` with no `kind:`, or a `kind:` other than "video". *(Built 2026-10-24.)* |
| `media-src-invalid` | `Component` | `Error` | `Static` | A `Media` with no `src:`, or one that is not a text literal or a list of one to four, or whose file is not mp4 or webm, or is named twice, or is on another host. *(Built 2026-10-24.)* |
| `media-poster-invalid` | `Component` | `Error` | `Static` | A `poster:` that is not the path of an image of the site. *(Built 2026-10-24.)* |
| `media-label-missing` | `Component` | `Error` | `Static` | A `Media` without a static `label:`: it is the name of the player and the text of the link a browser without video shows. *(Built 2026-10-24.)* |
| `media-text-alternative-missing` | `Component` | `Error` | `Static` | A `Media` with neither `caption:` nor `transcript:`. *(Built 2026-10-24.)* |
| `media-reference-invalid` | `Component` | `Error` | `Static` | A `caption:` or `transcript:` that is not the `id:` of an element of the program, or a `caption:` that names an element that is not a `Caption`. *(Built 2026-10-24.)* |
| `media-child-invalid` | `Component` | `Error` | `Static` | A `Media` with something in it. *(Built 2026-10-24.)* |
| `grid-min-invalid` | `Component` | `Error` | `Static` | A `Grid` with no `min:`, or one that is not an integer literal from 8 to 60 (rem). *(Built 2026-10-24.)* |
| `grid-weights-invalid` | `Component` | `Error` | `Static` | A `weights:` that is not a list of two to four number literals from 1 to 4 with at most two decimals, or a Grid with weights whose children are not written out one for each weight (a `For` or an `If` among them). *(Built 2026-10-24.)* |
| `asset-missing` | `Component` | `Error` | `Static` | A file that an `Image`, a `Media` or a `page { .. }` names and that is not a file inside the directory of the program (a symbolic link out of it counts as not there). *(Built 2026-10-24, found by the build and by `check`.)* |
| `asset-too-large` | `Component` | `Error` | `Static` | A file of the site over 20 MB (20,971,520 bytes). *(Built 2026-10-24.)* |
| `asset-type-mismatch` | `Component` | `Error` | `Static` | A file whose first bytes are not those of the format its extension names (png, jpg, jpeg, gif, webp, avif, mp4, webm). *(Built 2026-10-24.)* |
| `island-name-invalid` | `Component` | `Error` | `Static` | An `island` whose name is not in PascalCase. *(Built 2026-10-25, docs/islands.md.)* |
| `island-name-conflict` | `Component` | `Error` | `Static` | An `island` named like a built-in element (`Text`, `Image`, ..) or like a component of the file: an island is written like an element, so it needs a name of its own. *(Built 2026-10-25, docs/islands.md.)* |
| `island-duplicate` | `Component` | `Error` | `Static` | Two islands of one name, in one file or in two files of the program. *(Built 2026-10-25, docs/islands.md.)* |
| `island-entry-duplicate` | `Component` | `Error` | `Static` | `label`, `decorative`, `width`, `height`, `style`, `props` or `events` written twice in one island. *(Built 2026-10-25, docs/islands.md.)* |
| `island-file-duplicate` | `Component` | `Error` | `Static` | One path named twice among the scripts and the style of an island. *(Built 2026-10-25, docs/islands.md.)* |
| `island-path-invalid` | `Component` | `Error` | `Static` | A script or style path that is an address, is not relative to the program, climbs out of its directory, holds a character other than letters, digits, `.`, `_`, `-` and `/`, or is not `.js` (a script) or `.css` (a style). *(Built 2026-10-25, docs/islands.md.)* |
| `island-hash-invalid` | `Component` | `Error` | `Static` | A `sha256:` that is not 64 lowercase hexadecimal digits. *(Built 2026-10-25, docs/islands.md.)* |
| `island-script-missing` | `Component` | `Error` | `Static` | An island with no `script:`. *(Built 2026-10-25, docs/islands.md.)* |
| `island-too-many` | `Component` | `Error` | `Static` | An island with more than 8 scripts, 16 props or 16 events. *(Built 2026-10-25, docs/islands.md.)* |
| `island-size-missing` | `Component` | `Error` | `Static` | An island without both `width:` and `height:`: they give the frame its shape before it loads. *(Built 2026-10-25, docs/islands.md.)* |
| `island-size-invalid` | `Component` | `Error` | `Static` | A `width:` or `height:` outside 1..4000 pixels. *(Built 2026-10-25, docs/islands.md.)* |
| `island-label-missing` | `Component` | `Error` | `Static` | An island with neither a non-empty `label:` nor `decorative: true;`. *(Built 2026-10-25, docs/islands.md.)* |
| `island-label-conflict` | `Component` | `Error` | `Static` | An island with a `label:` and `decorative: true;`: a decorative frame is hidden from a screen reader, so its label would be read by no one. *(Built 2026-10-25, docs/islands.md.)* |
| `island-prop-name` | `Component` | `Error` | `Static` | A prop not named in lower case with underscores. *(Built 2026-10-25, docs/islands.md.)* |
| `island-prop-duplicate` | `Component` | `Error` | `Static` | A prop declared twice. *(Built 2026-10-25, docs/islands.md.)* |
| `island-prop-type` | `Component` | `Error` | `Static` | A prop whose type is not `Bool`, `Int`, `Float`, `String` or `Pointer`; or, on an instance, a literal of another type than the prop's. *(Built 2026-10-25, docs/islands.md.)* |
| `island-pointer-twice` | `Component` | `Error` | `Static` | Two `Pointer` props in one island: the runtime feeds one. *(Built 2026-10-25, docs/islands.md.)* |
| `island-event-name` | `Component` | `Error` | `Static` | An event not named in PascalCase: it is heard by the handler of the same name. *(Built 2026-10-25, docs/islands.md.)* |
| `island-event-duplicate` | `Component` | `Error` | `Static` | An event declared twice. *(Built 2026-10-25, docs/islands.md.)* |
| `island-event-type` | `Component` | `Error` | `Static` | An event that carries anything but a `Bool`, `Int`, `Float` or `String`, or nothing. *(Built 2026-10-25, docs/islands.md.)* |
| `island-source-missing` | `Component` | `Error` | `Static` | A file of an island that is not a file inside the directory of the program (a symbolic link out of it counts as not there), or that cannot be read. *(Built 2026-10-25, docs/islands.md.)* |
| `island-source-too-large` | `Component` | `Error` | `Static` | A file of an island over 512 kB. *(Built 2026-10-25, docs/islands.md.)* |
| `island-too-large` | `Component` | `Error` | `Static` | The files of one island over 1 MB together. *(Built 2026-10-25, docs/islands.md.)* |
| `island-hash-mismatch` | `Component` | `Error` | `Static` | A file whose SHA-256 is not the one the program pins it to. The message prints both. *(Built 2026-10-25, docs/islands.md.)* |
| `island-source-not-text` | `Component` | `Error` | `Static` | A script or style that is not UTF-8. *(Built 2026-10-25, docs/islands.md.)* |
| `island-source-unsafe` | `Component` | `Error` | `Static` | A script that holds `</script`, `<!--` or a NUL, or a style that holds `</style` or a NUL: copied byte for byte into the document of the frame it would end its own element, or hide a comment. Write `<\/script>` in a string. *(Built 2026-10-25, docs/islands.md.)* |
| `island-instance-children` | `Component` | `Error` | `Static` | An island instance with children: an island draws itself, from its own files. *(Built 2026-10-25, docs/islands.md.)* |
| `island-instance-label` | `Component` | `Error` | `Static` | An island instance with a `label:` or a `role:`: its label is written once, where the island is declared. *(Built 2026-10-25, docs/islands.md.)* |
| `island-prop-unknown` | `Component` | `Error` | `Static` | A binding on an island instance that is not one of the island's props and is not `on:` (or, on an open island, `text:`). *(Built 2026-10-25, docs/islands.md.)* |
| `island-prop-pointer-bound` | `Component` | `Error` | `Static` | A binding of the `Pointer` prop: the runtime feeds it, at most 20 a second, while the frame is on screen. *(Built 2026-10-25, docs/islands.md.)* |
| `island-prop-bound-twice` | `Component` | `Error` | `Static` | A prop written twice on one instance. *(Built 2026-10-25, docs/islands.md.)* |
| `island-prop-dynamic` | `Component` | `Error` | `Static` | A prop of a **sealed** island's instance that is not a literal (read from state): a sealed island's props are written where it is used. An open island's props may be state fields. *(Built 2026-10-25, docs/islands.md.)* |
| `island-prop-missing` | `Component` | `Error` | `Static` | An instance that does not give a prop (other than the `Pointer`). *(Built 2026-10-25, docs/islands.md.)* |
| `island-event-undeclared` | `Component` | `Error` | `Static` | `on: Name` on an instance where the island declares no event `Name`. *(Built 2026-10-25, docs/islands.md.)* |
| `island-event-args` | `Component` | `Error` | `Static` | `on: Name(args)` on an island instance: the event arrives with its own value as the handler's argument. *(Built 2026-10-25, docs/islands.md.)* |
| `island-event-handler-missing` | `Component` | `Error` | `Static` | `on: Name` where the component has no `on Name` handler. *(Built 2026-10-25, docs/islands.md.)* |
| `island-event-arity` | `Component` | `Error` | `Static` | A handler whose parameter count is not the island event's: one for an event that carries a value, none for one that does not. *(Built 2026-10-25, docs/islands.md.)* |
| `island-event-bound-twice` | `Component` | `Error` | `Static` | `on: Name` written twice on one instance. *(Built 2026-10-25, docs/islands.md.)* |
| `island-open-needs-dom` | `Component` | `Error` | `Static` | An instance of an open island (`sealed: false;`) in a component that does not declare `requires Dom;`: the island runs plain JavaScript in the page, with the page's authority, and the component says so. *(Built 2026-10-25, docs/islands.md section 12.)* |
| `island-open-pointer` | `Component` | `Error` | `Static` | An open island with a `Pointer` prop: the runtime feeds only a sealed frame; an open island is in the page and listens to the pointer itself. *(Built 2026-10-25, docs/islands.md section 12.)* |
| `island-text-alternative-missing` | `Component` | `Error` | `Static` | An open island that is not `decorative: true;` and whose instance has no `text: "id"`: its drawing is not readable by a screen reader, so the element that says the same in text is named. *(Built 2026-10-25, docs/islands.md section 12.)* |
| `island-text-alternative-invalid` | `Component` | `Error` | `Static` | A `text:` on an open island that is not a string literal, or that names an `id:` no element of the component has. *(Built 2026-10-25, docs/islands.md section 12.)* |
| `island-prop-source-unknown` | `Component` | `Error` | `Static` | A prop of an open island that is not a literal and not a `state` field of the component (a loop variable, say). *(Built 2026-10-25, docs/islands.md section 12.)* |
| `island-open-instance-twice` | `Component` | `Error` | `Static` | An open island instantiated twice in one component with props read from state: the page's script gives a state value to an island by its name. *(Built 2026-10-25, docs/islands.md section 12.)* |
| `page-duplicate` | `Component` | `Error` | `Static` | A second `page { .. }` in a file, or one in the entry file and one in an imported module. *(Built 2026-10-25, docs/page.md.)* |
| `page-entry-unknown` | `Component` | `Error` | `Static` | An entry of a `page { .. }` that is not `title`, `lang`, `description`, `share_description`, `canonical`, `image`, `icon`, `touch_icon`, `alternate` or `skip_link`; an attribute an entry does not take (`alt` is for `image`, `title` for `alternate`). *(Built 2026-10-25, docs/page.md.)* |
| `page-entry-duplicate` | `Component` | `Error` | `Static` | An entry written twice that a page has once, or an attribute written twice on one entry. *(Built 2026-10-25, docs/page.md.)* |
| `page-title-missing` | `Component` | `Error` | `Static` | A page with no `title:`, or a blank one. *(Built 2026-10-25, docs/page.md.)* |
| `page-lang-missing` | `Component` | `Error` | `Static` | A page with no `lang:`. *(Built 2026-10-25, docs/page.md.)* |
| `page-lang-invalid` | `Component` | `Error` | `Static` | A `lang:` that is not a language tag (two or three letters, then parts of one to eight letters or digits). *(Built 2026-10-25, docs/page.md.)* |
| `page-description-length` | `Component` | `Error` | `Static` | A `description:` or `share_description:` that is blank or over 300 characters. (`www.md` 5.3 said 50 to 160; the real page's description is 261 characters, so the bound was corrected in place.) *(Built 2026-10-25, docs/page.md.)* |
| `page-text-invalid` | `Component` | `Error` | `Static` | A title, description, alt text, alternate title or skip link text with a control character: a line break in a `<meta>` is another meta. *(Built 2026-10-25, docs/page.md.)* |
| `page-canonical-not-https` | `Component` | `Error` | `Static` | A `canonical:` that does not start with `https://`. *(Built 2026-10-25, docs/page.md.)* |
| `page-canonical-invalid` | `Component` | `Error` | `Static` | An `https://` canonical address with no host, a user name, a space, control character, quote, angle bracket, backslash or `#fragment`, or a host that is not a lower-case name. *(Built 2026-10-25, docs/page.md.)* |
| `page-image-alt-missing` | `Component` | `Error` | `Static` | An `image:` with no `alt:`, or a blank one. *(Built 2026-10-25, docs/page.md.)* |
| `page-image-needs-canonical` | `Component` | `Error` | `Static` | An `image:` with no `canonical:`: the card's absolute address is the file's path joined to the directory of the canonical address. *(Built 2026-10-25, docs/page.md.)* |
| `page-image-size-unreadable` | `Component` | `Error` | `Static` | A card image whose width and height cannot be read from its header (PNG, JPEG and WebP are read). *(Built 2026-10-25, docs/page.md.)* |
| `page-file-invalid` | `Component` | `Error` | `Static` | A file of a page (`image`, `icon`, `touch_icon`, `alternate`) written as an address, an absolute path, with `..`, with characters outside letters, digits, `.`, `_`, `-`, `/`, or with an extension its entry does not take. *(Built 2026-10-25, docs/page.md.)* |
| `page-alternate-title-missing` | `Component` | `Error` | `Static` | An `alternate:` with no `title:`, or a blank one. *(Built 2026-10-25, docs/page.md.)* |
| `page-skip-link-empty` | `Component` | `Error` | `Static` | A blank `skip_link:`. *(Built 2026-10-25, docs/page.md.)* |
| `skip-link-target-missing` | `Component` | `Error` | `Static` | A `skip_link:` in a program with no `Main`, or whose `Main` has no `id:`. *(Built 2026-10-25, docs/page.md.)* |
| `skip-link-target-dynamic` | `Component` | `Error` | `Static` | A `skip_link:` whose `Main` is not in the page's HTML: inside an `If` (inside a `For` it is `anchor-duplicate`, an id is one element), or in a component other than the one mounted into the page. *(Built 2026-10-25, docs/page.md.)* |
| `header-sticky-invalid` | `Component` | `Error` | `Static` | A `sticky:` on a `Header` that is not the literal `true` or `false`. *(Built 2026-10-25, docs/page.md.)* |
| `header-sticky-duplicate` | `Component` | `Error` | `Static` | A second sticky `Header` in a program. *(Built 2026-10-25, docs/page.md.)* |
| `header-sticky-misplaced` | `Component` | `Error` | `Static` | A sticky `Header` inside a `Section`, `Article`, `Aside`, `Main`, `Nav`, `Footer`, `Figure`, `Details` or another `Header`: it is not the banner and would stick only as far as its parent goes. *(Built 2026-10-25, docs/page.md.)* |
| `island-event-dropped` | `Component` | `Info` | `Runtime` | A message from an island frame that the page did not hear: an event the island does not declare or the view does not hear, a value of another type than the one declared (or a `String` over 4096 characters), a frame that sent more than 50 in a second, or a pointer message that is not two finite numbers. Counted in `window.__cui_island_stats.dropped`; nothing is thrown and no handler runs. A rule of the runtime, not a refusal of the build. *(Built 2026-10-25, docs/islands.md.)* |
| `parse-nesting-too-deep` | `Component` | `Error` | `Static` | `if` statements nested more than 32 deep, or expressions, view nodes and types nested more than 32 deep together (an operand of a parenthesis, a call, a list, a `match` arm, a lambda, an `else if`, a view node inside a view node, a type argument; a chain of binary operators counts one for each operator): a parse error, not a stack overflow. *(Statements: issue #93. Expressions, views and types: issue #99, `docs/nesting.md`.)* |
| `import-chain-too-deep` | `Component` | `Error` | `Static` | A chain of imports longer than 32 files (each file imports the next). *(Issue #99, `docs/nesting.md` section 5. The other `import-*` refusals are not in this table yet.)* |
| `table-child-invalid` | `Component` | `Error` | `Static` | A `Table` child that is not a `Row` or a `For` that makes `Row`s. *(Built 2026-10-21, issue #42, section 4.)* |
| `capability-missing` | `Capability` | `Error` | `Static` | A component using an effect without `requires Capability` |
| `capability-unused` | `Capability` | `Error` | `Static` | A `requires` declaration with no reachable use |
| `capability-insufficient` | `Capability` | `Error` | `Static` | A `requires` narrower than the effect's needs |
| `capability-narrowing-illegal` | `Capability` | `Error` | `Static` | A bound on a capability that takes none, a `Credential` with no name or an illegal name or host, a `Network` bound that is not `name[:port]`, or a `Location` bound that is not `"coarse"` or `"fine"`. *(A bound that is not a string literal is `parse-unexpected-token`; built 2026-10-20, `docs/adapters.md` section 10.)* |
| `credential-ambiguous` | `Capability` | `Error` | `Static` | A component declaring more than one `Credential`: a call cannot choose between them. *(Built 2026-10-21, `docs/adapters.md` section 18.)* |
| `credential-host-not-allowed` | `Capability` | `Error` | `Static` | A host named in `Credential("name", "host")` that the component's `Network` does not allow. *(Built 2026-10-21.)* |
| `credential-needs-mount-options` | `Capability` | `Error` | `Static` | A build with auto-mount (or `--pwa`) of a component declaring `Credential` that never calls `credential.set`: nothing can hand it a token; use `--no-auto-mount` and `mount_X(root, { credentials })`. *(Built 2026-10-21.)* |
| `credential-name-invalid` | `Capability` | `Error` | `Static` | A `credential.set(..)` or `credential.clear(..)` whose first argument is not a string literal naming the component's declared `Credential`, or in a component that declares none. *(Built 2026-10-23, issue #83, `docs/bearer-token.md`.)* |
| `capability-bound-violated` | `Capability` | `Error` | `Static` | A `net.get` of a literal URL whose host the component's `Network` does not allow (the runtime guard answers `Err("blocked: ...")` for the URLs it cannot see) |
| `schedule-event-unknown` | `Schedule` | `Error` | `Static` | A `start` or `every` naming an event the component does not declare |
| `schedule-event-params` | `Schedule` | `Error` | `Static` | A `start` or `every` naming an event that takes parameters |
| `schedule-period-invalid` | `Schedule` | `Error` | `Static` | An `every` period outside 100..86,400,000 ms |
| `schedule-duplicate` | `Schedule` | `Error` | `Static` | The same `start`, or the same `every` period and event, twice in a component |
| `accessibility-role-undefined` | `Accessibility` | `Error` | `Static` | A view primitive without a `role` |
| `accessibility-label-empty` | `Accessibility` | `Error` | `Static` | An interactive primitive with empty `label` |
| `accessibility-keyboard-missing` | `Accessibility` | `Error` | `Static` | An interactive primitive without a keyboard action for Enter |
| `unsupported-on-target` | `Target` | `Error` | `Static` | A `target` block built for a target not in the build |
| `target-not-in-build` | `Target` | `Error` | `Static` | A `target` block for a target not in the build configuration |
| `target-divergence-unproved` | `Target` | `Error` | `Static` | (Phase 4) Two `target` blocks whose semantics differ in a way the build cannot prove equivalent |
| `foreign-scope-missing` | `Foreign` | `Error` | `Static` | A `foreign` function without a `from "scope"` |
| `foreign-interface-missing` | `Foreign` | `Error` | `Static` | A program calling a foreign function without an adapter |
| `name-reserved` | `Ir` | `Error` | `Static` | A binding named `component`, `state`, `view`, etc. |
| `evidence-over-claim` | `Ir` | `Error` | `Tested` | A report claiming a guarantee the implementation does not enforce |
| `ir-version-unsupported` | `Ir` | `Error` | `Runtime` | An IR consumer does not recognize the schema version |
| `internal` | `Ir` | `Error` | `Unknown` | A backend or compiler internal error (mirrors Cancho Core's `internal`) |

### 3.2 Replay rules (`cancho-ui test`)

`cancho-ui test` reports failures in its own JSON shape
([`docs/replay.md`](../../docs/replay.md) §7), tier `tested`, with these stable
tags: `test-state-mismatch`, `test-expect-mismatch`, `test-fault-missing`, `test-fault-mismatch`, `trace-no-assertions`,
`trace-malformed`, `trace-credential-unknown`, `trace-schema-unsupported`, `trace-unknown-key`,
`trace-bad-step`, `trace-bad-value`, `trace-unknown-component`,
`trace-unknown-event`, `trace-params-arity`, `trace-value-too-long`, `trace-value-not-an-option`, `trace-unknown-state-field`,
`trace-unknown-request`, `trace-request-not-pending`,
`trace-request-not-scriptable`, `trace-bad-response`,
`trace-event-after-dispose`, `trace-navigate-invalid`, `trace-history-empty`, `trace-value-not-a-file`, `trace-stream-state`, `trace-locale-invalid`, `trace-move-invalid`, `trace-number-invalid`, `replay-unsupported`, `replay-tick-limit`, `replay-route-limit`, `replay-type-error`,
`replay-unknown-identifier`, `replay-no-match-arm`, `replay-non-finite`,
`replay-number-format`. (`replay-non-finite` is also what the emitted page throws for the same result: `docs/semantics.md`,
"Arithmetic is finite".) Their meanings are in `docs/replay.md` §7.1. A program that
does not compile is still refused with the §3 rule that refuses it (exit `1`).

### 3.1 What is not in the catalog

- **No warnings.** A diagnostic is `Error`. A warning would
  either be ignored or acted on; if ignored it is not a
  guarantee, if acted on it is a refusal. The same discipline
  Cancho Core applies — "a diagnostic is a refusal."
- **No stylistic hints.** "Consider this instead" messages are
  refused as a category; they go in `remediation.suggestion`
  with `confidence: Low`.
- **No "this may be undefined" hedge.** The Phase 1 contract
  is either the language defines it, or it is `unknown`. A
  refusal that says "may" is refused with `evidence-over-claim`.

---

## 4. The `--output json` shape

The JSON form of a diagnostic is what an agent reads.

```json
{
  "code": "capability-missing",
  "severity": "Error",
  "tier": "Static",
  "subject": {
    "kind": "Capability",
    "name": "Network"
  },
  "span": {
    "file": "examples/agent_tasks/add_capability.cui",
    "start": { "line": 12, "column": 5 },
    "end": { "line": 12, "column": 35 }
  },
  "message": "Component 'SearchForm' performs an effect that requires Network but does not declare it. Add `requires Network` to the component header.",
  "expected": "requires Network",
  "actual": "absent",
  "remediation": {
    "description": "Add `requires Network` to the component header.",
    "suggestion": null,
    "confidence": "High"
  },
  "related": [],
  "notes": []
}
```

### 4.1 Stable field names

Field names do not change between schema versions. A new
field is additive (minor version bump); a renamed field is a
breaking change (major version bump). Consumers ignore unknown
fields.

### 4.2 Stable field order

The serialization order is the declaration order in §2.
Consumers can rely on it for diff stability.

### 4.3 No embedded newlines in `message`

The `message` field is one line. A multi-line explanation goes
in `notes[]`, with one note per entry.

---

## 5. The text form

The human-readable form is parallel. `cancho-ui check` without
`--output json` prints one block per diagnostic:

```
error[capability-missing]: Component 'SearchForm' performs an
effect that requires Network but does not declare it. Add
`requires Network` to the component header.
   --> examples/agent_tasks/add_capability.cui:12:5
   |
12 |     let r = net.request("GET", "/api/items").await()
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
help: Add `requires Network` to the component header.
   |
 2 | component SearchForm {
 3 |     requires Network
   |     ++++++++++++++++
```

The text form is **derived from the JSON form**, not the other
way around. The same discipline Cancho Core's
[`agent-cli.md`](https://github.com/alpibrusl/cancho/blob/main/docs/agent-cli.md)
follows: the binary's output is the source of truth.

---

## 6. The evidence report

`cancho-ui report` produces a JSON document with five sections,
one per tier. Each section is a list of structured items.

```json
{
  "schema": "cancho-ui-report",
  "version": "0.1.0",
  "static": {
    "checks_run": ["capability-missing", "state-not-owned", ...],
    "checks_passed": 42,
    "checks_failed": 1,
    "diagnostics": [
      // structured Diagnostic objects
    ]
  },
  "tested": {
    "tests_run": ["counter_increments", "search_cancels_on_dispose", ...],
    "tests_passed": 17,
    "tests_failed": 0,
    "details": [
      // structured test results
    ]
  },
  "runtime": {
    "checks_performed": ["capability-grant", "dispose-cancellation", ...],
    "checks_passed": 31,
    "checks_failed": 0,
    "details": []
  },
  "trust": {
    "boundaries": [
      { "from": "browser", "symbol": "fetch", "tier": "Trust" },
      { "from": "browser:storage", "symbol": "localStorage.getItem", "tier": "Trust" }
    ],
    "count": 2
  },
  "unknown": {
    "items": [
      { "description": "Cross-component event ordering", "tier": "Unknown" }
    ]
  }
}
```

### 6.1 What the report never contains

- A `trust` boundary labeled as anything other than `Trust`.
- A `static` check that did not actually run (it goes in
  `unknown`).
- A `tested` test that was skipped (it is listed with
  `outcome: "skipped"`, not as `passed`).
- An `evidence-over-claim` item; that is a refusal against the
  report itself, caught by the conformance suite.

---

## 7. The semantic-diff output

> **Corrected in place (Phase 3).** The sketch below was written before
> `cancho-ui diff` existed and is **not** the shape it emits. The built shape
> is specified in [`docs/semantic-diff.md`](../docs/semantic-diff.md) §3:
> `capabilities` / `components` / `authority` (`expanded | reduced |
> unchanged`) replace the flat `added_capabilities` lists;
> `changed_transitions` with prose `from` / `to` is replaced by per-event
> `writes_added`, `writes_removed` and a rendered `body`, because a prose
> description of a transition is not derivable from the IR;
> `added_foreign_dependencies` is not emitted because the IR has no foreign
> nodes yet (the report says so in `not_compared`); `changed_accessibility`
> is folded into the per-component `view` element list. Exit code `5` means
> "differs" (`1` stays "refused"). The sketch is kept below as the
> original intent.

`cancho-ui diff <base> <candidate>` produces a JSON document
listing the semantic changes between two IRs.

```json
{
  "schema": "cancho-ui-diff",
  "version": "0.1.0",
  "added_capabilities": ["Network"],
  "removed_capabilities": [],
  "changed_transitions": [
    {
      "component": "SearchForm",
      "event": "SearchRequested",
      "from": "synchronous local search",
      "to": "async net.search with cancellation",
      "tier_change": null
    }
  ],
  "added_foreign_dependencies": [
    { "from": "browser", "symbol": "AbortController" }
  ],
  "changed_accessibility": [
    { "component": "SearchForm", "from": "role: textbox", "to": "role: search" }
  ],
  "added_components": [],
  "removed_components": ["OldSearchForm"]
}
```

A normal source diff is parallel (line-by-line). The semantic
diff is what an agent uses to decide whether a change is
safe to apply; the source diff is for a human to read.

---

## 8. Versioning

| Artifact | Schema | Version | Bump policy |
|---|---|---|---|
| Diagnostic | `cancho-ui-diagnostic` | `0.1.0` (Phase 1) | Major = breaking field; minor = additive |
| Evidence report | `cancho-ui-report` | `0.1.0` (Phase 1) | Same |
| Semantic diff | `cancho-ui-diff` | `0.1.0` (Phase 2) | Same |

A consumer that does not recognize the schema or version
refuses with `ir-version-unsupported` for IR inputs and
`schema-version-unsupported` for report / diff inputs.

---

## 9. What Phase 1 must add to this sketch

The Phase 1 delta is small and explicit:

- The full rule catalog in §3, each rule with at least one
  refusal fixture in `examples/refusal/`.
- The JSON serializer for `Diagnostic` in §4.
- The text formatter for §5, derived from the JSON form.
- A `cancho-ui report --output json` command that produces
  the shape in §6.
- A `tests/diagnostics.rs` snapshot test that pins every
  fixture's diagnostic output, so the schema does not drift.

Anything else is Phase 2+. Phase 1 does not add a rule that is
not in §3, and does not modify a rule's `code` without a major
schema bump.