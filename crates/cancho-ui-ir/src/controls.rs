//! The bindings of the built-in elements (docs/controls.md): which keys exist, and what `disabled:` may be.
//!
//! Before this pass an unknown key on `Column`, `Text`, `Button` or `TextField` was accepted and either dropped or
//! written as an HTML attribute, so `Button { disabled: true }` compiled and did nothing (issue #35).

use crate::ir::*;
use crate::lower::LowerError;

/// The keys a built-in element takes. `label`, `role`, `on`, `value`, `disabled` and `submit` have meaning;
/// `placeholder` is written as the attribute of that name.
pub const ELEMENT_KEYS: &[&str] = &[
    "label",
    "role",
    "on",
    "value",
    "disabled",
    "placeholder",
    "submit",
    "tone",
    "gap",
    "max_width",
    "type",
    "autocomplete",
    "inputmode",
    "maxlength",
    "name",
    "error",
    "rows",
    "checked",
    "options",
    "to",
];

/// What a `NavLink` takes (docs/routing.md section 4); every other key is `binding-unknown` on it.
pub const NAVLINK_KEYS: &[&str] = &["label", "role", "to"];

/// What a `Select` takes (docs/controls.md section 10); every other key is `binding-unknown` on it.
pub const SELECT_KEYS: &[&str] = &[
    "label", "role", "on", "value", "options", "disabled", "error", "name",
];

/// What a `Checkbox` takes (docs/controls.md section 9); every other key is `binding-unknown` on it.
pub const CHECKBOX_KEYS: &[&str] = &[
    "label", "role", "on", "checked", "disabled", "error", "name",
];

/// The `rows:` range of a `TextArea`: how many lines it shows.
pub const ROWS_RANGE: (i64, i64) = (1, 30);

/// The `type:` of a `TextField` (docs/controls.md section 5): the HTML input types that hold one line of text and give the
/// handler a string. `number` is left out on purpose (measured: `1e5` typed key by key ends as `5`).
pub const INPUT_TYPES: &[&str] = &["text", "password", "email", "search", "tel", "url"];

/// The tones of a status text (docs/presentation.md section 3).
pub const TONES: &[&str] = &["ok", "warn", "error"];
/// The `gap:` steps of a `Row` or `Column` (section 2), and the `max_width:` range in characters.
pub const GAP_MAX: i64 = 4;
pub const MAX_WIDTH_RANGE: (i64, i64) = (10, 200);

pub fn check_controls(ir: &IrFile) -> Result<(), LowerError> {
    for c in &ir.components {
        check_node(c, &c.view.body)?;
    }
    Ok(())
}

fn check_node(c: &IrComponent, node: &IrViewNode) -> Result<(), LowerError> {
    match node {
        IrViewNode::Element {
            kind,
            accessibility,
            bindings,
            children,
            span,
        } => {
            if !matches!(kind, IrElementKind::Named(_)) {
                for b in bindings {
                    check_binding(c, kind, b)?;
                }
                check_navlink_complete(kind, accessibility, bindings, *span)?;
                check_checkbox_complete(kind, bindings, *span)?;
                check_select_complete(kind, bindings, *span)?;
            }
            for ch in children {
                check_node(c, ch)?;
            }
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            for ch in children {
                check_node(c, ch)?;
            }
        }
        IrViewNode::Text { .. } => {}
    }
    Ok(())
}

fn element_name(kind: &IrElementKind) -> &'static str {
    match kind {
        IrElementKind::Column => "Column",
        IrElementKind::Row => "Row",
        IrElementKind::Table => "Table",
        IrElementKind::TableRow => "a row of a Table",
        IrElementKind::Cell | IrElementKind::HeaderCell => "a cell of a Table",
        IrElementKind::Text => "Text",
        IrElementKind::Button => "Button",
        IrElementKind::TextField => "TextField",
        IrElementKind::TextArea => "TextArea",
        IrElementKind::Checkbox => "Checkbox",
        IrElementKind::Select => "Select",
        IrElementKind::NavLink => "NavLink",
        IrElementKind::Named(_) => "a component",
    }
}

/// `on: Event` and `on: Event(args)` (docs/row-actions.md section 2): the event is declared by the component, a click
/// gives it exactly the arguments the view writes, and a field gives it the typed value.
fn check_on(c: &IrComponent, kind: &IrElementKind, b: &IrBinding) -> Result<(), LowerError> {
    let (name, args) = match &b.value {
        IrBindingValue::EventRef(n) => (n, None),
        IrBindingValue::EventCall { name, args } => (name, Some(args.len())),
        _ => return Ok(()),
    };
    let Some(event) = c.events.iter().find(|e| &e.name == name) else {
        let known: Vec<&str> = c.events.iter().map(|e| e.name.as_str()).collect();
        return Err(LowerError::ControlRefused {
            code: "event-unknown",
            message: format!(
                "`on: {name}`: this component has no `on {name}` handler (it has {})",
                if known.is_empty() {
                    "none".to_string()
                } else {
                    known.join(", ")
                }
            ),
            span: b.span,
        });
    };
    if args.is_some() && !matches!(kind, IrElementKind::Button) {
        return Err(LowerError::ControlRefused {
            code: "event-args-unsupported",
            message: format!(
                "`on: {name}(..)` gives a click its arguments, so it is a binding of Button, not of {}: a field gives its handler the typed value (`on: {name}`)",
                element_name(kind)
            ),
            span: b.span,
        });
    }
    let given = match (args, kind) {
        (Some(n), _) => n,
        // A click gives no arguments; a field gives the value it holds.
        // A field gives the text it holds, a checkbox whether it is checked now.
        (
            None,
            IrElementKind::TextField
            | IrElementKind::TextArea
            | IrElementKind::Checkbox
            | IrElementKind::Select,
        ) => event.params.len().min(1),
        (None, _) => 0,
    };
    if event.params.len() != given {
        return Err(LowerError::ControlRefused {
            code: "event-arity-mismatch",
            message: format!(
                "`on {name}` takes {} parameter(s) and this binding gives it {given}{}",
                event.params.len(),
                if args.is_none()
                    && matches!(kind, IrElementKind::Button)
                    && !event.params.is_empty()
                {
                    format!("; a click has no arguments, so write them: `on: {name}(..)`")
                } else {
                    String::new()
                }
            ),
            span: b.span,
        });
    }
    Ok(())
}

/// `tone:` is a binding of a `Text` (or the cell it becomes in a table): `"ok"`, `"warn"`, `"error"` or a `String` state field.
fn check_tone(c: &IrComponent, kind: &IrElementKind, b: &IrBinding) -> Result<(), LowerError> {
    if !matches!(
        kind,
        IrElementKind::Text | IrElementKind::Cell | IrElementKind::HeaderCell
    ) {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`tone:` is a binding of Text (a status text), not of {}",
                element_name(kind)
            ),
            span: b.span,
        });
    }
    let bad = |message: String| LowerError::ControlRefused {
        code: "tone-invalid",
        message,
        span: b.span,
    };
    match &b.value {
        IrBindingValue::String(s) if TONES.contains(&s.as_str()) => Ok(()),
        IrBindingValue::String(s) => Err(bad(format!(
            "`tone: \"{s}\"`: the tones are {}",
            TONES.join(", ")
        ))),
        IrBindingValue::StateRef(name) => {
            if c.state
                .iter()
                .any(|f| &f.name == name && f.ty == IrType::String)
            {
                Ok(())
            } else {
                Err(bad(format!(
                    "`tone: {name}` needs a `String` state field named `{name}` in this component (its value is one of {})",
                    TONES.join(", ")
                )))
            }
        }
        _ => Err(bad(format!(
            "`tone:` takes {} as a string, or the name of a `String` state field",
            TONES.join(", ")
        ))),
    }
}

/// The `autocomplete:` of a `TextField` (docs/controls.md section 6): `on`, `off`, or one field name of the HTML autofill list that
/// is not a contact-section or address form of its own (a single token; `shipping given-name` is not one).
pub const AUTOCOMPLETE: &[&str] = &[
    "on",
    "off",
    "name",
    "honorific-prefix",
    "given-name",
    "additional-name",
    "family-name",
    "honorific-suffix",
    "nickname",
    "username",
    "new-password",
    "current-password",
    "one-time-code",
    "organization-title",
    "organization",
    "street-address",
    "address-line1",
    "address-line2",
    "address-line3",
    "address-level4",
    "address-level3",
    "address-level2",
    "address-level1",
    "country",
    "country-name",
    "postal-code",
    "cc-name",
    "cc-given-name",
    "cc-additional-name",
    "cc-family-name",
    "cc-number",
    "cc-exp",
    "cc-exp-month",
    "cc-exp-year",
    "cc-csc",
    "cc-type",
    "transaction-currency",
    "transaction-amount",
    "language",
    "bday",
    "bday-day",
    "bday-month",
    "bday-year",
    "sex",
    "url",
    "photo",
    "tel",
    "tel-country-code",
    "tel-national",
    "tel-area-code",
    "tel-local",
    "tel-extension",
    "email",
    "impp",
];

/// The `inputmode:` keywords of a `TextField`: which keyboard a phone shows.
pub const INPUT_MODES: &[&str] = &[
    "none", "text", "decimal", "numeric", "tel", "search", "email", "url",
];

/// The `maxlength:` range, in UTF-16 code units (what the browser counts).
pub const MAXLENGTH_RANGE: (i64, i64) = (1, 100_000);

/// The longest `name:` of a field, and the characters it may hold.
pub const NAME_MAX: usize = 64;

/// `rows:` is a binding of a `TextArea` and takes an integer literal from 1 to 30.
fn check_rows(kind: &IrElementKind, b: &IrBinding) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::TextArea) {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`rows:` is a binding of TextArea (how many lines it shows), not of {}",
                element_name(kind)
            ),
            span: b.span,
        });
    }
    match &b.value {
        IrBindingValue::Const { expr, .. } if matches!(**expr, IrExpr::IntLit { value, .. } if (ROWS_RANGE.0..=ROWS_RANGE.1).contains(&value)) => {
            Ok(())
        }
        _ => Err(LowerError::ControlRefused {
            code: "input-attribute-invalid",
            message: format!(
                "`rows:` takes an integer literal from {} to {}",
                ROWS_RANGE.0, ROWS_RANGE.1
            ),
            span: b.span,
        }),
    }
}

/// `error:` is a binding of a control that holds a value (docs/controls.md section 7) and names a `String` state field: the
/// message that goes with the control, empty while the control is valid.
fn check_error(c: &IrComponent, kind: &IrElementKind, b: &IrBinding) -> Result<(), LowerError> {
    if !kind.is_field() && !matches!(kind, IrElementKind::Checkbox | IrElementKind::Select) {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`error:` is a binding of TextField, TextArea, Checkbox and Select (the message that goes with a control), not of {}",
                element_name(kind)
            ),
            span: b.span,
        });
    }
    match &b.value {
        IrBindingValue::StateRef(name)
            if c.state
                .iter()
                .any(|f| &f.name == name && f.ty == IrType::String) =>
        {
            Ok(())
        }
        _ => Err(LowerError::ControlRefused {
            code: "error-binding-invalid",
            message: "`error:` takes the name of a `String` state field of this component, e.g. `error: user_msg`; the field is invalid while it holds text and valid while it is empty".to_string(),
            span: b.span,
        }),
    }
}

/// `autocomplete:`, `inputmode:`, `maxlength:` and `name:` are bindings of a `TextField` with a checked value.
fn check_text_attribute(kind: &IrElementKind, b: &IrBinding) -> Result<(), LowerError> {
    if !kind.is_field()
        && !(b.key == "name" && matches!(kind, IrElementKind::Checkbox | IrElementKind::Select))
    {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`{}:` is a binding of TextField and TextArea, not of {}",
                b.key,
                element_name(kind)
            ),
            span: b.span,
        });
    }
    let bad = |message: String| LowerError::ControlRefused {
        code: "input-attribute-invalid",
        message,
        span: b.span,
    };
    match (b.key.as_str(), &b.value) {
        ("autocomplete", IrBindingValue::String(s)) if AUTOCOMPLETE.contains(&s.as_str()) => Ok(()),
        ("autocomplete", _) => Err(bad(
            "`autocomplete:` takes `on`, `off` or one field name of the HTML autofill list as a string literal, e.g. `\"username\"`, `\"current-password\"`, `\"new-password\"`, `\"one-time-code\"`, `\"email\"`, `\"tel\"`, `\"given-name\"`".to_string(),
        )),
        ("inputmode", IrBindingValue::String(s)) if INPUT_MODES.contains(&s.as_str()) => Ok(()),
        ("inputmode", _) => Err(bad(format!(
            "`inputmode:` takes one of {} as a string literal",
            INPUT_MODES.join(", ")
        ))),
        ("maxlength", IrBindingValue::Const { expr, .. })
            if matches!(**expr, IrExpr::IntLit { value, .. } if (MAXLENGTH_RANGE.0..=MAXLENGTH_RANGE.1).contains(&value)) =>
        {
            Ok(())
        }
        ("maxlength", _) => Err(bad(format!(
            "`maxlength:` takes an integer literal from {} to {} (UTF-16 code units, as the browser counts)",
            MAXLENGTH_RANGE.0, MAXLENGTH_RANGE.1
        ))),
        ("name", IrBindingValue::String(s)) if valid_name(s) => Ok(()),
        _ => Err(bad(format!(
            "`name:` takes a string literal of at most {NAME_MAX} letters, digits, `-`, `_` or `.`, starting with a letter"
        ))),
    }
}

fn valid_name(s: &str) -> bool {
    let mut chars = s.chars();
    s.len() <= NAME_MAX
        && chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// `type:` is a binding of a `TextField` and takes one of `INPUT_TYPES` as a string literal.
fn check_input_type(kind: &IrElementKind, b: &IrBinding) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::TextField) {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`type:` is a binding of TextField (`type: \"password\"`), not of {}",
                element_name(kind)
            ),
            span: b.span,
        });
    }
    match &b.value {
        IrBindingValue::String(s) if INPUT_TYPES.contains(&s.as_str()) => Ok(()),
        _ => Err(LowerError::ControlRefused {
            code: "input-type-invalid",
            message: format!(
                "`type:` takes one of {} as a string literal; `number` is not one (a number field answers an empty string until what is typed is a number, and the page draws the field again at every key, so `1e5` typed key by key ends as `5`), and a checkbox, a choice or a date is a different control",
                INPUT_TYPES.join(", ")
            ),
            span: b.span,
        }),
    }
}

/// `gap:` and `max_width:` are bindings of a `Column` or a `Row`, and take an integer literal in a fixed range.
fn check_layout(kind: &IrElementKind, b: &IrBinding) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::Column | IrElementKind::Row) {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`{}:` is a binding of Column and Row, not of {}",
                b.key,
                element_name(kind)
            ),
            span: b.span,
        });
    }
    let (lo, hi) = if b.key == "gap" {
        (0, GAP_MAX)
    } else {
        MAX_WIDTH_RANGE
    };
    match &b.value {
        IrBindingValue::Const { expr, .. } if matches!(**expr, IrExpr::IntLit { value, .. } if (lo..=hi).contains(&value)) => {
            Ok(())
        }
        _ => Err(LowerError::ControlRefused {
            code: "layout-value-invalid",
            message: format!(
                "`{}:` takes an integer literal from {lo} to {hi}{}",
                b.key,
                if b.key == "gap" {
                    " (a step of 0.25, 0.5, 1 and 2 rem after 0)"
                } else {
                    " (a width in characters; the page is centred in it)"
                }
            ),
            span: b.span,
        }),
    }
}

/// `checked:` is a binding of a `Checkbox` and names a `Bool` state field: the box is on while it is true.
fn check_checked(c: &IrComponent, kind: &IrElementKind, b: &IrBinding) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::Checkbox) {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`checked:` is a binding of Checkbox, not of {}",
                element_name(kind)
            ),
            span: b.span,
        });
    }
    match &b.value {
        IrBindingValue::StateRef(name)
            if c.state
                .iter()
                .any(|f| &f.name == name && f.ty == IrType::Bool) =>
        {
            Ok(())
        }
        _ => Err(LowerError::ControlRefused {
            code: "checked-binding-invalid",
            message: "`checked:` takes the name of a `Bool` state field of this component, e.g. `checked: agreed`; the box is on while the field is true".to_string(),
            span: b.span,
        }),
    }
}

/// A `Checkbox` is drawn from `checked:` and reports through `on:`; without either it would be a box the page forgets at the
/// next redraw, or one that tells the program nothing.
fn check_checkbox_complete(
    kind: &IrElementKind,
    bindings: &[IrBinding],
    span: cancho_ui_syntax::span::Span,
) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::Checkbox) {
        return Ok(());
    }
    for (key, what) in [
        ("checked", "the `Bool` it shows"),
        ("on", "the event it reports to"),
    ] {
        if !bindings.iter().any(|b| b.key == key) {
            return Err(LowerError::ControlRefused {
                code: "checkbox-incomplete",
                message: format!(
                    "a Checkbox needs `{key}:`, {what}: `Checkbox {{ label: \"..\" checked: flag on: Toggle }}`"
                ),
                span,
            });
        }
    }
    Ok(())
}

/// `options:` is a binding of a `Select`: a list literal of string literals, or the name of a `List` state field.
fn check_options(c: &IrComponent, kind: &IrElementKind, b: &IrBinding) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::Select) {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`options:` is a binding of Select, not of {}",
                element_name(kind)
            ),
            span: b.span,
        });
    }
    let bad = |message: &str| LowerError::ControlRefused {
        code: "select-binding-invalid",
        message: message.to_string(),
        span: b.span,
    };
    match &b.value {
        IrBindingValue::StateRef(name)
            if c.state
                .iter()
                .any(|f| &f.name == name && matches!(f.ty, IrType::List { .. })) =>
        {
            Ok(())
        }
        IrBindingValue::Const { expr, .. } => match &**expr {
            IrExpr::ListLit { items, .. }
                if items.iter().all(|i| matches!(i, IrExpr::StringLit { .. })) =>
            {
                Ok(())
            }
            _ => Err(bad(
                "`options:` takes a list of string literals, e.g. `options: [\"open\", \"doing\", \"done\"]`, or the name of a `List` state field",
            )),
        },
        _ => Err(bad(
            "`options:` takes a list of string literals, e.g. `options: [\"open\", \"doing\", \"done\"]`, or the name of a `List` state field",
        )),
    }
}

/// The `value:` of a `Select` names the `String` state field that holds the choice.
fn check_select_value(c: &IrComponent, b: &IrBinding) -> Result<(), LowerError> {
    match &b.value {
        IrBindingValue::StateRef(name)
            if c.state
                .iter()
                .any(|f| &f.name == name && f.ty == IrType::String) =>
        {
            Ok(())
        }
        _ => Err(LowerError::ControlRefused {
            code: "select-binding-invalid",
            message: "a Select's `value:` takes the name of a `String` state field of this component, which holds the chosen option"
                .to_string(),
            span: b.span,
        }),
    }
}

/// A `Select` needs the list it chooses from, the field that holds the choice and the event it reports to.
fn check_select_complete(
    kind: &IrElementKind,
    bindings: &[IrBinding],
    span: cancho_ui_syntax::span::Span,
) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::Select) {
        return Ok(());
    }
    for (key, what) in [
        ("options", "the list it chooses from"),
        ("value", "the `String` field that holds the choice"),
        ("on", "the event it reports to"),
    ] {
        if !bindings.iter().any(|b| b.key == key) {
            return Err(LowerError::ControlRefused {
                code: "select-incomplete",
                message: format!(
                    "a Select needs `{key}:`, {what}: `Select {{ label: \"..\" options: [\"a\", \"b\"] value: choice on: Pick }}`"
                ),
                span,
            });
        }
    }
    Ok(())
}

/// A `NavLink` has a name to read and a path to point at (docs/routing.md section 4).
fn check_navlink_complete(
    kind: &IrElementKind,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    span: cancho_ui_syntax::span::Span,
) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::NavLink) {
        return Ok(());
    }
    let has_label = accessibility.label.is_some() || accessibility.label_expr.is_some();
    if !has_label || !bindings.iter().any(|b| b.key == "to") {
        return Err(LowerError::ControlRefused {
            code: "navlink-incomplete",
            message: "a NavLink needs `label:`, the text of the link, and `to:`, the path it points at: `NavLink { label: \"Item 3\" to: \"/items/3\" }`".to_string(),
            span,
        });
    }
    Ok(())
}

/// `to:` is a binding of NavLink, and a literal one is a path the URL fragment can carry.
fn check_to(kind: &IrElementKind, b: &IrBinding) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::NavLink) {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`to:` is a binding of NavLink (the path it points at), not of {}",
                element_name(kind)
            ),
            span: b.span,
        });
    }
    if let IrBindingValue::Const { expr, .. } = &b.value
        && let IrExpr::StringLit { value, .. } = expr.as_ref()
        && !crate::routing::path_is_legal(value)
    {
        return Err(LowerError::ControlRefused {
            code: "navlink-target-invalid",
            message: format!(
                "`to: {value:?}` is not a path: it starts with `/` and holds no space, `#` or character outside printable ASCII, at most {} characters",
                crate::routing::PATH_MAX
            ),
            span: b.span,
        });
    }
    Ok(())
}

fn check_binding(c: &IrComponent, kind: &IrElementKind, b: &IrBinding) -> Result<(), LowerError> {
    if matches!(kind, IrElementKind::NavLink)
        && !NAVLINK_KEYS.contains(&b.key.as_str())
        && ELEMENT_KEYS.contains(&b.key.as_str())
    {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`{}:` is not a binding of NavLink; it takes {}",
                b.key,
                NAVLINK_KEYS.join(", ")
            ),
            span: b.span,
        });
    }
    if b.key == "to" {
        check_to(kind, b)?;
    }
    if matches!(kind, IrElementKind::Select)
        && !SELECT_KEYS.contains(&b.key.as_str())
        && ELEMENT_KEYS.contains(&b.key.as_str())
    {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`{}:` is not a binding of Select; it takes {}",
                b.key,
                SELECT_KEYS.join(", ")
            ),
            span: b.span,
        });
    }
    if matches!(kind, IrElementKind::Select) && b.key == "value" {
        check_select_value(c, b)?;
    }
    if matches!(kind, IrElementKind::Checkbox)
        && !CHECKBOX_KEYS.contains(&b.key.as_str())
        && ELEMENT_KEYS.contains(&b.key.as_str())
    {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`{}:` is not a binding of Checkbox; it takes {}",
                b.key,
                CHECKBOX_KEYS.join(", ")
            ),
            span: b.span,
        });
    }
    if !ELEMENT_KEYS.contains(&b.key.as_str()) {
        return Err(LowerError::ControlRefused {
            code: "binding-unknown",
            message: format!(
                "`{}:` is not a binding of {}; it would be dropped or written as a stray attribute. The keys are {}",
                b.key,
                element_name(kind),
                ELEMENT_KEYS.join(", ")
            ),
            span: b.span,
        });
    }
    if b.key == "on" {
        check_on(c, kind, b)?;
    }
    if b.key == "tone" {
        check_tone(c, kind, b)?;
    }
    if b.key == "gap" || b.key == "max_width" {
        check_layout(kind, b)?;
    }
    if b.key == "type" {
        check_input_type(kind, b)?;
    }
    if matches!(
        b.key.as_str(),
        "autocomplete" | "inputmode" | "maxlength" | "name"
    ) {
        check_text_attribute(kind, b)?;
    }
    if b.key == "error" {
        check_error(c, kind, b)?;
    }
    if b.key == "rows" {
        check_rows(kind, b)?;
    }
    if b.key == "checked" {
        check_checked(c, kind, b)?;
    }
    if b.key == "options" {
        check_options(c, kind, b)?;
    }
    if b.key == "submit" {
        if !matches!(kind, IrElementKind::TextField) {
            return Err(LowerError::ControlRefused {
                code: "binding-unknown",
                message: format!(
                    "`submit:` is a binding of TextField (Enter in the field fires the event), not of {}{}",
                    element_name(kind),
                    if matches!(kind, IrElementKind::TextArea) {
                        "; in a TextArea Enter starts a new line"
                    } else {
                        ""
                    }
                ),
                span: b.span,
            });
        }
        let IrBindingValue::EventRef(name) = &b.value else {
            return Err(LowerError::ControlRefused {
                code: "submit-event-invalid",
                message: "`submit:` names an event of the component, e.g. `submit: Apply`"
                    .to_string(),
                span: b.span,
            });
        };
        match c.events.iter().find(|e| &e.name == name) {
            None => {
                return Err(LowerError::ControlRefused {
                    code: "submit-event-invalid",
                    message: format!("`submit: {name}`: this component has no `on {name}` handler"),
                    span: b.span,
                });
            }
            Some(e) if !e.params.is_empty() => {
                return Err(LowerError::ControlRefused {
                    code: "submit-event-invalid",
                    message: format!(
                        "`submit: {name}`: `on {name}` takes parameters, and Enter has none to give; use an event without parameters"
                    ),
                    span: b.span,
                });
            }
            Some(_) => {}
        }
    }
    if b.key == "disabled" {
        let interactive = kind.is_focusable();
        if !interactive {
            return Err(LowerError::ControlRefused {
                code: "binding-unknown",
                message: format!(
                    "`disabled:` is a binding of Button, TextField, TextArea, Checkbox and Select, not of {}",
                    element_name(kind)
                ),
                span: b.span,
            });
        }
        match &b.value {
            IrBindingValue::Const { expr, .. } if matches!(**expr, IrExpr::BoolLit { .. }) => {}
            IrBindingValue::StateRef(name) => {
                let is_bool = c
                    .state
                    .iter()
                    .any(|f| &f.name == name && f.ty == IrType::Bool);
                if !is_bool {
                    return Err(LowerError::ControlRefused {
                        code: "disabled-not-bool",
                        message: format!(
                            "`disabled: {name}` needs a `Bool` state field named `{name}` in this component"
                        ),
                        span: b.span,
                    });
                }
            }
            _ => {
                return Err(LowerError::ControlRefused {
                    code: "disabled-not-bool",
                    message:
                        "`disabled:` takes `true`, `false` or the name of a `Bool` state field"
                            .to_string(),
                    span: b.span,
                });
            }
        }
    }
    Ok(())
}
