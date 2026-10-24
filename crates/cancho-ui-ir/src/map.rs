//! `Map` (docs/maps.md): the bindings it takes and the rule tags that guard them. A `Map` draws the 3 x 3 tiles around the tile of `center:` from
//! the host `tiles:` names, pins the `places:` over them and writes the list of those places beside the picture. The host is the program's:
//! the compiler has no default and names no provider.

use cancho_ui_syntax::span::Span;

use crate::ir::*;
use crate::lower::LowerError;

/// What a `Map` takes; every other key is `binding-unknown` on it.
pub const MAP_KEYS: &[&str] = &[
    "label",
    "role",
    "center",
    "places",
    "zoom",
    "tiles",
    "attribution",
];
/// The keys that belong to a `Map` and to nothing else.
const OWN_KEYS: &[&str] = &["center", "places", "zoom", "tiles", "attribution"];

/// The highest zoom of a map (the tile sets serve 0 to 19).
pub const MAX_ZOOM: i64 = 19;
/// The highest zoom for a component that holds only a coarse position (a bare `requires Location`): a tile at least as large as the
/// rounding cell of 0.01 degree up to latitude 76 (docs/maps.md section 2).
pub const COARSE_MAX_ZOOM: i64 = 13;
/// The longest attribution, in characters.
pub const MAX_ATTRIBUTION: usize = 200;

fn refused(code: &'static str, message: String, span: Span) -> LowerError {
    LowerError::ControlRefused {
        code,
        message,
        span,
    }
}

/// Where the tiles come from: the origin to put in a Content-Security-Policy (`None`: the page's own) and the `host[:port]` a `requires
/// Network("..")` must name (`None` for the page's own origin, which a bare `requires Network;` covers).
#[derive(Debug, PartialEq, Eq)]
pub struct TileSource {
    pub origin: Option<String>,
    pub host: Option<String>,
}

/// Read a `tiles:` template: `https://host[:port]/path/{z}/{x}/{y}.png` (`http` only for `localhost` and `127.0.0.1`), or a path on the page's own
/// origin (`/tiles/{z}/{x}/{y}.png`, `./tiles/{z}/{x}/{y}.png`). Each of `{z}`, `{x}` and `{y}` once and no other brace; no space, quote, `<`,
/// `>`, backslash or `#`; no user name or password in the address.
pub fn tile_source(template: &str) -> Result<TileSource, String> {
    for p in ["{z}", "{x}", "{y}"] {
        if template.matches(p).count() != 1 {
            return Err(format!("it must hold {p} exactly once"));
        }
    }
    let braces = template.matches('{').count() + template.matches('}').count();
    if braces != 6 {
        return Err("it holds a brace that is not {z}, {x} or {y}".to_string());
    }
    if template
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || "\"'<>\\#".contains(c))
    {
        return Err("it holds a space, a quote, `<`, `>`, a backslash or `#`".to_string());
    }
    if let Some(rest) = template
        .strip_prefix('/')
        .filter(|r| !r.starts_with('/'))
        .or_else(|| template.strip_prefix("./"))
    {
        if rest.is_empty() {
            return Err("it names no path".to_string());
        }
        return Ok(TileSource {
            origin: None,
            host: None,
        });
    }
    let (scheme, rest) = if let Some(r) = template.strip_prefix("https://") {
        ("https", r)
    } else if let Some(r) = template.strip_prefix("http://") {
        ("http", r)
    } else {
        return Err(
            "it must start with `https://` (or `http://` for localhost), or be a path on this page's own origin (`/tiles/{z}/{x}/{y}.png`)"
                .to_string(),
        );
    };
    let authority = rest.split(['/', '?']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return Err("it names no host, or holds a user name or password".to_string());
    }
    let host = authority.to_ascii_lowercase();
    let (name, port) = match host.split_once(':') {
        Some((n, p)) => (n, Some(p)),
        None => (host.as_str(), None),
    };
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        || port.is_some_and(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(
            "its host is not a name (letters, digits, `-` and `.`) with an optional port"
                .to_string(),
        );
    }
    if scheme == "http" && name != "localhost" && name != "127.0.0.1" {
        return Err(
            "`http://` is for localhost only: a tile host on the internet is `https://`"
                .to_string(),
        );
    }
    Ok(TileSource {
        origin: Some(format!("{scheme}://{host}")),
        host: Some(host),
    })
}

/// Does a view hold a `Map`?
pub fn view_has_map(node: &IrViewNode) -> bool {
    match node {
        IrViewNode::Element { kind, children, .. } => {
            matches!(kind, IrElementKind::Map) || children.iter().any(view_has_map)
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            children.iter().any(view_has_map)
        }
        IrViewNode::Text { .. } => false,
    }
}

/// Every `Map` of the view: its `tiles:` text.
pub fn tile_templates(node: &IrViewNode, out: &mut Vec<String>) {
    match node {
        IrViewNode::Element {
            kind,
            bindings,
            children,
            ..
        } => {
            if matches!(kind, IrElementKind::Map) {
                for b in bindings {
                    if let ("tiles", IrBindingValue::String(s)) = (&*b.key, &b.value) {
                        out.push(s.clone());
                    }
                }
            }
            for c in children {
                tile_templates(c, out);
            }
        }
        IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
            for c in children {
                tile_templates(c, out);
            }
        }
        IrViewNode::Text { .. } => {}
    }
}

/// The origins an HTML page must allow images from for the maps of `ir` (no duplicates, in order of first use): what the generated page's
/// Content-Security-Policy lists after `'self' data:`.
pub fn image_origins(ir: &IrFile) -> Vec<String> {
    let mut templates = Vec::new();
    for c in &ir.components {
        tile_templates(&c.view.body, &mut templates);
    }
    let mut out: Vec<String> = Vec::new();
    for t in templates {
        if let Ok(TileSource {
            origin: Some(o), ..
        }) = tile_source(&t)
            && !out.contains(&o)
        {
            out.push(o);
        }
    }
    out
}

/// The `img-src` of the Content-Security-Policy of a generated HTML page, when the program draws a `Map` (`None`: no map, no policy): the page's
/// own origin, `data:` and the origins of the tile hosts the maps name. An image from any other host is not requested at all (measured,
/// docs/maps.md section 2).
pub fn csp_img_sources(ir: &IrFile) -> Option<String> {
    let mut any = false;
    for c in &ir.components {
        any |= view_has_map(&c.view.body);
    }
    any.then(|| {
        let mut s = String::from("'self' data:");
        for o in image_origins(ir) {
            s.push(' ');
            s.push_str(&o);
        }
        s
    })
}

/// The highest zoom this component may ask for: 13 for a bare `requires Location` (a position rounded to 2 decimals), 19 otherwise.
pub fn max_zoom(c: &IrComponent) -> i64 {
    let coarse = c.requires.iter().any(|r| {
        r.name == "Location" && (r.bounds.is_empty() || r.bounds.iter().all(|b| b == "coarse"))
    });
    if coarse { COARSE_MAX_ZOOM } else { MAX_ZOOM }
}

/// The key of one binding against the element it is on.
pub fn check_binding(
    c: &IrComponent,
    kind: &IrElementKind,
    b: &IrBinding,
) -> Result<(), LowerError> {
    let is_map = matches!(kind, IrElementKind::Map);
    if !is_map {
        if OWN_KEYS.contains(&b.key.as_str()) {
            return Err(refused(
                "binding-unknown",
                format!("`{}:` is a binding of Map, not of this element", b.key),
                b.span,
            ));
        }
        return Ok(());
    }
    if !MAP_KEYS.contains(&b.key.as_str()) {
        return Err(refused(
            "binding-unknown",
            format!(
                "`{}:` is not a binding of Map; it takes {}",
                b.key,
                MAP_KEYS.join(", ")
            ),
            b.span,
        ));
    }
    let field = |name: &str| c.state.iter().find(|f| f.name == name);
    match (b.key.as_str(), &b.value) {
        ("center", IrBindingValue::String(_)) => Ok(()),
        ("center", IrBindingValue::StateRef(n))
            if field(n).is_some_and(|f| matches!(f.ty, IrType::String)) =>
        {
            Ok(())
        }
        ("center", _) => Err(refused(
            "map-binding-invalid",
            "a Map's `center:` is a `String` state field holding \"lat,lon\" (what `location.current()` answers), or that text"
                .to_string(),
            b.span,
        )),
        ("places", IrBindingValue::StateRef(n))
            if field(n).is_some_and(|f| matches!(f.ty, IrType::List { .. })) =>
        {
            Ok(())
        }
        ("places", _) => Err(refused(
            "map-binding-invalid",
            "a Map's `places:` is a `List` state field of rows like {\"name\":\"Sol\",\"pos\":\"40.4169,-3.7035\"}, the places it pins and lists"
                .to_string(),
            b.span,
        )),
        ("zoom", IrBindingValue::Const { expr, .. })
            if matches!(**expr, IrExpr::IntLit { value, .. } if (0..=max_zoom(c)).contains(&value)) =>
        {
            Ok(())
        }
        ("zoom", _) => Err(refused(
            "map-zoom-invalid",
            format!(
                "a Map's `zoom:` is a whole number from 0 to {}{}",
                max_zoom(c),
                if max_zoom(c) < MAX_ZOOM {
                    " (this component holds a position rounded to 1 km, `requires Location`: a tile finer than that says more than it knows; ask for `Location(\"fine\")` to go to 19)"
                } else {
                    ""
                }
            ),
            b.span,
        )),
        ("tiles", IrBindingValue::String(s)) => tile_source(s).map(|_| ()).map_err(|why| {
            refused(
                "map-tiles-invalid",
                format!("a Map's `tiles:` is not a tile address: {why}"),
                b.span,
            )
        }),
        ("tiles", _) => Err(refused(
            "map-tiles-invalid",
            "a Map's `tiles:` is a text literal like \"https://tiles.example/{z}/{x}/{y}.png\" (or a path on this page: \"/tiles/{z}/{x}/{y}.png\"): the host is the program's and is written where it can be read"
                .to_string(),
            b.span,
        )),
        ("attribution", IrBindingValue::String(s))
            if !s.trim().is_empty() && s.chars().count() <= MAX_ATTRIBUTION =>
        {
            Ok(())
        }
        ("attribution", _) => Err(refused(
            "map-binding-invalid",
            format!("a Map's `attribution:` is a text literal of 1 to {MAX_ATTRIBUTION} characters, shown under the map"),
            b.span,
        )),
        _ => Ok(()),
    }
}

/// A `Map` has a name, a centre, the places it lists, a zoom, a tile address and an attribution; its tiles come from a host the component has
/// declared (`requires Network("host")`, or a bare `requires Network;` for a path on this page).
pub fn check_complete(
    c: &IrComponent,
    kind: &IrElementKind,
    accessibility: &IrAccessibility,
    bindings: &[IrBinding],
    span: Span,
) -> Result<(), LowerError> {
    if !matches!(kind, IrElementKind::Map) {
        return Ok(());
    }
    if accessibility.label.is_none() && accessibility.label_expr.is_none() {
        return Err(refused(
            "map-incomplete",
            "a Map needs `label:`, the name of the picture a screen reader announces".to_string(),
            span,
        ));
    }
    for (key, what) in [
        ("center", "the position the map is centred on, \"lat,lon\""),
        (
            "places",
            "the list of places, the accessible form of the map",
        ),
        ("zoom", "the zoom, a whole number"),
        (
            "tiles",
            "the tile address, a text literal with {z}, {x} and {y}",
        ),
        (
            "attribution",
            "the credit the tile provider asks for, shown under the map",
        ),
    ] {
        if !bindings.iter().any(|b| b.key == key) {
            return Err(refused(
                "map-incomplete",
                format!("a Map needs `{key}:`, {what}"),
                span,
            ));
        }
    }
    let template = bindings.iter().find_map(|b| match (&*b.key, &b.value) {
        ("tiles", IrBindingValue::String(s)) => Some((s, b.span)),
        _ => None,
    });
    if let Some((t, at)) = template
        && let Ok(source) = tile_source(t)
    {
        let network = c.requires.iter().find(|r| r.name == "Network");
        let declared = match (&source.host, network) {
            (_, None) => false,
            (None, Some(_)) => true,
            (Some(h), Some(n)) => n.bounds.iter().any(|b| b.to_ascii_lowercase() == *h),
        };
        if !declared {
            let line = match &source.host {
                Some(h) => format!("requires Network(\"{h}\");"),
                None => "requires Network;".to_string(),
            };
            return Err(refused(
                "map-host-undeclared",
                format!(
                    "this Map asks {} for its tiles, and the component does not declare it: add `{line}` to the component (the tile host is authority, and the semantic diff reports it as a change)",
                    source.host.as_deref().unwrap_or("this page's own origin")
                ),
                at,
            ));
        }
    }
    Ok(())
}
