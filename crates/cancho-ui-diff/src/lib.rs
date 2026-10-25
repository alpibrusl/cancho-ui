//! cancho-ui-diff — the semantic diff of two typed IRs.
//!
//! Pure and total: `diff_ir` takes two IRs and returns a report; it
//! cannot fail and has no I/O. The design, the JSON shape, the exit
//! codes and what the diff does not claim are in
//! `docs/semantic-diff.md`.

mod human;
mod render;

use std::collections::{BTreeMap, BTreeSet};

use cancho_ui_ir::ir::{IrComponent, IrFile, IrScheduleKind, IrStmt};
use cancho_ui_ir::serialize::IrSpan;
use serde::Serialize;

pub use human::render_human;

pub const SCHEMA: &str = "cancho-ui-diff";
pub const VERSION: &str = "0.1.0";

/// Categories this version does not inspect. Printed in every report so
/// a `changed: false` is never read as a claim about them.
pub const NOT_COMPARED: &[&str] = &[
    "foreign_boundaries",
    "properties",
    "lifecycle",
    "target_overrides",
    "event_ordering_and_cancellation",
    "view_structure_and_order",
    "view_text_content",
    "expression_value_equivalence",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Authority {
    Expanded,
    Reduced,
    Unchanged,
}

impl<T> Delta<T> {
    fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }
}

impl Authority {
    pub fn as_str(self) -> &'static str {
        match self {
            Authority::Expanded => "expanded",
            Authority::Reduced => "reduced",
            Authority::Unchanged => "unchanged",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiffReport {
    pub schema: &'static str,
    pub version: &'static str,
    pub base: String,
    pub candidate: String,
    pub changed: bool,
    pub authority: Authority,
    pub change_count: usize,
    pub capabilities: CapabilityChanges,
    pub components: ComponentChanges,
    /// The settings of the program's `theme { .. }` that differ (docs/theme.md section 7): one entry per colour token, font key, or whole
    /// scheme added or removed. A recolour is a change of the program and is reported as its own line; it has no authority, so
    /// `authority` is unaffected. Absent from the JSON when the themes are equal.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub theme: Vec<ThemeChange>,
    /// The settings of the program's `page { .. }` that differ (docs/page.md section 6): the title, the description, the canonical address, the preview
    /// image, the icons, the alternates, the language. A page has no authority: `authority` is unaffected. Absent from the JSON when the pages are equal.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub page: Vec<PageChange>,
    /// The files the views name (`Image`, `Media`; docs/structure.md section 10) that were added, removed or changed (a different SHA-256).
    /// Absent from the JSON when they are the same files.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<AssetChange>,
    /// The settings of the sealed islands that differ (docs/islands.md section 6): an island added or removed, a file with a different SHA-256, a
    /// changed label, size, prop or event. A sealed island reaches nothing but the page, so none of it changes `authority`; it is reported first,
    /// because it is code the review has not read. Absent from the JSON when the islands are equal.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub islands: Vec<IslandChange>,
    pub not_compared: Vec<&'static str>,
}

/// One setting of a theme: `light.accent`, `dark.bg`, `font.body.sha256`. `base` is `None` when the candidate adds it, `candidate` is `None`
/// when it removes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThemeChange {
    pub setting: String,
    pub base: Option<String>,
    pub candidate: Option<String>,
}

/// One setting of a page: `title`, `image.width`, `alternate.agents.md.mime`. `None` on the side that does not have it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageChange {
    pub setting: String,
    pub base: Option<String>,
    pub candidate: Option<String>,
}

/// One file a view names: `base` and `candidate` are its SHA-256, `None` when the side does not use it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetChange {
    pub path: String,
    pub base: Option<String>,
    pub candidate: Option<String>,
}

/// One setting of an island: `Hero`, `Hero.script assets/scene.js`, `Hero.prop seed`. `None` on the side that does not have it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IslandChange {
    pub setting: String,
    pub base: Option<String>,
    pub candidate: Option<String>,
}

fn diff_islands(base: &IrFile, candidate: &IrFile) -> Vec<IslandChange> {
    let flat = |f: &IrFile| -> BTreeMap<String, String> {
        f.islands
            .iter()
            .flat_map(cancho_ui_ir::island::flatten)
            .collect()
    };
    let (b, c) = (flat(base), flat(candidate));
    let mut keys: BTreeSet<&String> = b.keys().collect();
    keys.extend(c.keys());
    keys.into_iter()
        .filter(|k| b.get(*k) != c.get(*k))
        .map(|k| IslandChange {
            setting: k.clone(),
            base: b.get(k).cloned(),
            candidate: c.get(k).cloned(),
        })
        .collect()
}

fn diff_assets(base: &IrFile, candidate: &IrFile) -> Vec<AssetChange> {
    let map = |f: &IrFile| -> BTreeMap<String, String> {
        f.assets
            .iter()
            .map(|a| (a.path.clone(), a.sha256.clone()))
            .collect()
    };
    let (b, c) = (map(base), map(candidate));
    let mut keys: BTreeSet<&String> = b.keys().collect();
    keys.extend(c.keys());
    keys.into_iter()
        .filter(|k| b.get(*k) != c.get(*k))
        .map(|k| AssetChange {
            path: k.clone(),
            base: b.get(k).cloned(),
            candidate: c.get(k).cloned(),
        })
        .collect()
}

fn diff_page(base: &IrFile, candidate: &IrFile) -> Vec<PageChange> {
    let flat = |f: &IrFile| -> BTreeMap<String, String> {
        f.page
            .as_ref()
            .map(|p| cancho_ui_ir::page::flatten(p).into_iter().collect())
            .unwrap_or_default()
    };
    let (b, c) = (flat(base), flat(candidate));
    let mut keys: BTreeSet<&String> = b.keys().collect();
    keys.extend(c.keys());
    keys.into_iter()
        .filter(|k| b.get(*k) != c.get(*k))
        .map(|k| PageChange {
            setting: k.clone(),
            base: b.get(k).cloned(),
            candidate: c.get(k).cloned(),
        })
        .collect()
}

fn diff_theme(base: &IrFile, candidate: &IrFile) -> Vec<ThemeChange> {
    let flat = |f: &IrFile| -> BTreeMap<String, String> {
        f.theme
            .as_ref()
            .map(|t| cancho_ui_ir::theme::flatten(t).into_iter().collect())
            .unwrap_or_default()
    };
    let (b, c) = (flat(base), flat(candidate));
    let mut keys: BTreeSet<&String> = b.keys().collect();
    keys.extend(c.keys());
    keys.into_iter()
        .filter(|k| b.get(*k) != c.get(*k))
        .map(|k| ThemeChange {
            setting: k.clone(),
            base: b.get(k).cloned(),
            candidate: c.get(k).cloned(),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CapabilityChanges {
    pub added: Vec<CapabilityChange>,
    pub removed: Vec<CapabilityChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct CapabilityChange {
    pub component: String,
    pub capability: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ComponentChanges {
    pub added: Vec<ComponentSummary>,
    pub removed: Vec<ComponentSummary>,
    pub changed: Vec<ComponentDiff>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ComponentSummary {
    pub name: String,
    pub requires: Vec<String>,
    pub state: Vec<String>,
    pub events: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ComponentDiff {
    pub name: String,
    pub authority: Authority,
    pub requires: Delta<String>,
    /// `start Event` and `every <ms> Event`, as written; a changed period is one removed and one added
    /// (docs/schedules.md section 5). A change in how often a component acts is a behavioural change.
    pub schedules: Delta<String>,
    pub state: StateChanges,
    pub events: EventChanges,
    pub effects: EffectChanges,
    pub view: Delta<String>,
    /// The hosts the component's `Link`s name (docs/structure.md): where a reader can be sent. A new host is a review item; it is not
    /// an effect, so `authority` does not move. Absent from the JSON when nothing changed.
    #[serde(skip_serializing_if = "Delta::is_empty")]
    pub links: Delta<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Delta<T> {
    pub added: Vec<T>,
    pub removed: Vec<T>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Change<T> {
    pub base: T,
    pub candidate: T,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StateChanges {
    pub added: Vec<StateEntry>,
    pub removed: Vec<StateEntry>,
    pub changed: Vec<StateChanged>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StateEntry {
    pub name: String,
    pub ty: String,
    pub initial: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StateChanged {
    pub name: String,
    pub ty: Option<Change<String>>,
    pub initial: Option<Change<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EventChanges {
    pub added: Vec<EventEntry>,
    pub removed: Vec<EventEntry>,
    pub changed: Vec<EventChanged>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EventEntry {
    pub name: String,
    pub params: Vec<String>,
    pub writes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EventChanged {
    pub name: String,
    pub params: Option<Change<Vec<String>>>,
    pub writes_added: Vec<String>,
    pub writes_removed: Vec<String>,
    pub body: Option<Change<Vec<String>>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EffectChanges {
    pub added: Vec<EffectEntry>,
    pub removed: Vec<EffectEntry>,
    pub changed: Vec<EffectCount>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EffectEntry {
    pub capability: String,
    pub path: String,
    pub spans: Vec<IrSpan>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EffectCount {
    pub capability: String,
    pub path: String,
    pub base_count: usize,
    pub candidate_count: usize,
}

/// A component reduced to the facets the diff compares.
struct Facets {
    requires: BTreeSet<String>,
    schedules: BTreeSet<String>,
    state: BTreeMap<String, (String, String)>,
    events: BTreeMap<String, EventFacet>,
    effects: BTreeMap<(String, String), Vec<IrSpan>>,
    view: BTreeMap<String, usize>,
    links: BTreeSet<String>,
}

#[derive(Default)]
struct EventFacet {
    params: Vec<String>,
    writes: BTreeSet<String>,
    body: Vec<String>,
}

fn facets(c: &IrComponent) -> Facets {
    let mut events: BTreeMap<String, EventFacet> = BTreeMap::new();
    for e in &c.events {
        // Two handlers of one name are merged: writes unioned, bodies
        // concatenated in source order, params from the first.
        let f = events.entry(e.name.clone()).or_default();
        if f.params.is_empty() {
            f.params = e.params.clone();
        }
        for s in IrStmt::leaves(&e.body) {
            if let IrStmt::Assign { target, .. } = s {
                f.writes.insert(target.clone());
            }
        }
        for s in &e.body {
            f.body.push(render::stmt(s));
        }
    }
    let mut effects: BTreeMap<(String, String), Vec<IrSpan>> = BTreeMap::new();
    for e in &c.reachable_effects {
        effects
            .entry((e.capability.clone(), e.path.join(".")))
            .or_default()
            .push(IrSpan::from(e.span));
    }
    for spans in effects.values_mut() {
        spans.sort_by_key(|s| (s.start.line, s.start.column, s.end.line, s.end.column));
    }
    let mut descs = Vec::new();
    render::view_descriptors(&c.view.body, &mut descs);
    let mut view = BTreeMap::new();
    for d in descs {
        *view.entry(d).or_insert(0usize) += 1;
    }
    Facets {
        schedules: c
            .schedules
            .iter()
            .map(|s| match s.kind {
                IrScheduleKind::Start => format!("start {}", s.event),
                IrScheduleKind::Every(ms) => format!("every {ms} {}", s.event),
                IrScheduleKind::Route => format!("route {}", s.event),
                IrScheduleKind::Subscribe => match &s.stream {
                    Some(t) => match &t.status {
                        Some(st) => format!("subscribe {} {:?} status {st}", s.event, t.url),
                        None => format!("subscribe {} {:?}", s.event, t.url),
                    },
                    None => format!("subscribe {}", s.event),
                },
            })
            .collect(),
        // A bound is part of the declared authority: `Network("a.example")` is not `Network`.
        requires: c
            .requires
            .iter()
            .map(|r| {
                if r.bounds.is_empty() {
                    r.name.clone()
                } else {
                    let bounds: Vec<String> = r
                        .bounds
                        .iter()
                        .map(|b| format!("\"{}\"", b.to_ascii_lowercase()))
                        .collect();
                    format!("{}({})", r.name, bounds.join(", "))
                }
            })
            .collect(),
        state: c
            .state
            .iter()
            .map(|s| {
                (
                    s.name.clone(),
                    (render::ty(&s.ty), render::expr(&s.initial)),
                )
            })
            .collect(),
        events,
        effects,
        view,
        links: cancho_ui_ir::structure::link_hosts(&c.view.body),
    }
}

fn summary(c: &IrComponent) -> ComponentSummary {
    let f = facets(c);
    ComponentSummary {
        name: c.name.clone(),
        requires: f.requires.into_iter().collect(),
        state: f.state.into_keys().collect(),
        events: f.events.into_keys().collect(),
    }
}

fn by_name(ir: &IrFile) -> BTreeMap<&str, &IrComponent> {
    let mut m = BTreeMap::new();
    for c in &ir.components {
        // Duplicate names cannot reach here from one checked file; across
        // an import the first declaration wins, deterministically.
        m.entry(c.name.as_str()).or_insert(c);
    }
    m
}

fn diff_component(name: &str, b: &IrComponent, c: &IrComponent) -> ComponentDiff {
    let (bf, cf) = (facets(b), facets(c));

    let requires = Delta {
        added: cf.requires.difference(&bf.requires).cloned().collect(),
        removed: bf.requires.difference(&cf.requires).cloned().collect(),
    };
    let schedules = Delta {
        added: cf.schedules.difference(&bf.schedules).cloned().collect(),
        removed: bf.schedules.difference(&cf.schedules).cloned().collect(),
    };
    let authority = if !requires.added.is_empty() {
        Authority::Expanded
    } else if !requires.removed.is_empty() {
        Authority::Reduced
    } else {
        Authority::Unchanged
    };

    let entry = |n: &String, v: &(String, String)| StateEntry {
        name: n.clone(),
        ty: v.0.clone(),
        initial: v.1.clone(),
    };
    let mut state = StateChanges {
        added: vec![],
        removed: vec![],
        changed: vec![],
    };
    for (n, v) in &cf.state {
        match bf.state.get(n) {
            None => state.added.push(entry(n, v)),
            Some(old) if old != v => state.changed.push(StateChanged {
                name: n.clone(),
                ty: (old.0 != v.0).then(|| Change {
                    base: old.0.clone(),
                    candidate: v.0.clone(),
                }),
                initial: (old.1 != v.1).then(|| Change {
                    base: old.1.clone(),
                    candidate: v.1.clone(),
                }),
            }),
            Some(_) => {}
        }
    }
    for (n, v) in &bf.state {
        if !cf.state.contains_key(n) {
            state.removed.push(entry(n, v));
        }
    }

    let ev_entry = |n: &String, e: &EventFacet| EventEntry {
        name: n.clone(),
        params: e.params.clone(),
        writes: e.writes.iter().cloned().collect(),
    };
    let mut events = EventChanges {
        added: vec![],
        removed: vec![],
        changed: vec![],
    };
    for (n, e) in &cf.events {
        match bf.events.get(n) {
            None => events.added.push(ev_entry(n, e)),
            Some(old) => {
                let params = (old.params != e.params).then(|| Change {
                    base: old.params.clone(),
                    candidate: e.params.clone(),
                });
                let writes_added: Vec<String> = e.writes.difference(&old.writes).cloned().collect();
                let writes_removed: Vec<String> =
                    old.writes.difference(&e.writes).cloned().collect();
                let body = (old.body != e.body).then(|| Change {
                    base: old.body.clone(),
                    candidate: e.body.clone(),
                });
                if params.is_some()
                    || !writes_added.is_empty()
                    || !writes_removed.is_empty()
                    || body.is_some()
                {
                    events.changed.push(EventChanged {
                        name: n.clone(),
                        params,
                        writes_added,
                        writes_removed,
                        body,
                    });
                }
            }
        }
    }
    for (n, e) in &bf.events {
        if !cf.events.contains_key(n) {
            events.removed.push(ev_entry(n, e));
        }
    }

    let mut effects = EffectChanges {
        added: vec![],
        removed: vec![],
        changed: vec![],
    };
    let eff = |k: &(String, String), spans: &Vec<IrSpan>| EffectEntry {
        capability: k.0.clone(),
        path: k.1.clone(),
        spans: spans.clone(),
    };
    for (k, spans) in &cf.effects {
        match bf.effects.get(k) {
            None => effects.added.push(eff(k, spans)),
            Some(old) if old.len() != spans.len() => effects.changed.push(EffectCount {
                capability: k.0.clone(),
                path: k.1.clone(),
                base_count: old.len(),
                candidate_count: spans.len(),
            }),
            Some(_) => {}
        }
    }
    for (k, spans) in &bf.effects {
        if !cf.effects.contains_key(k) {
            effects.removed.push(eff(k, spans));
        }
    }

    let mut view = Delta {
        added: vec![],
        removed: vec![],
    };
    let keys: BTreeSet<&String> = bf.view.keys().chain(cf.view.keys()).collect();
    for k in keys {
        let (bn, cn) = (
            bf.view.get(k).copied().unwrap_or(0),
            cf.view.get(k).copied().unwrap_or(0),
        );
        for _ in bn..cn {
            view.added.push(k.clone());
        }
        for _ in cn..bn {
            view.removed.push(k.clone());
        }
    }

    let links = Delta {
        added: cf.links.difference(&bf.links).cloned().collect(),
        removed: bf.links.difference(&cf.links).cloned().collect(),
    };
    ComponentDiff {
        name: name.to_string(),
        authority,
        requires,
        schedules,
        state,
        events,
        effects,
        view,
        links,
    }
}

fn leaf_count(d: &ComponentDiff) -> usize {
    d.requires.added.len()
        + d.requires.removed.len()
        + d.schedules.added.len()
        + d.schedules.removed.len()
        + d.state.added.len()
        + d.state.removed.len()
        + d.state.changed.len()
        + d.events.added.len()
        + d.events.removed.len()
        + d.events.changed.len()
        + d.effects.added.len()
        + d.effects.removed.len()
        + d.effects.changed.len()
        + d.view.added.len()
        + d.view.removed.len()
        + d.links.added.len()
        + d.links.removed.len()
}

/// Compare two typed IRs by component name. `base_label` and
/// `candidate_label` are echoed into the report (normally the paths as
/// typed on the command line).
pub fn diff_ir(
    base: &IrFile,
    candidate: &IrFile,
    base_label: &str,
    candidate_label: &str,
) -> DiffReport {
    let (bm, cm) = (by_name(base), by_name(candidate));
    let mut components = ComponentChanges {
        added: vec![],
        removed: vec![],
        changed: vec![],
    };
    let mut caps = CapabilityChanges {
        added: vec![],
        removed: vec![],
    };
    let (mut expanded, mut reduced) = (false, false);

    for (name, c) in &cm {
        match bm.get(name) {
            None => {
                let s = summary(c);
                for r in &s.requires {
                    caps.added.push(CapabilityChange {
                        component: s.name.clone(),
                        capability: r.clone(),
                    });
                }
                if !s.requires.is_empty() {
                    expanded = true;
                }
                components.added.push(s);
            }
            Some(b) => {
                let d = diff_component(name, b, c);
                if leaf_count(&d) > 0 {
                    for r in &d.requires.added {
                        caps.added.push(CapabilityChange {
                            component: d.name.clone(),
                            capability: r.clone(),
                        });
                    }
                    for r in &d.requires.removed {
                        caps.removed.push(CapabilityChange {
                            component: d.name.clone(),
                            capability: r.clone(),
                        });
                    }
                    expanded |= d.authority == Authority::Expanded;
                    reduced |= d.authority == Authority::Reduced;
                    components.changed.push(d);
                }
            }
        }
    }
    for (name, b) in &bm {
        if !cm.contains_key(name) {
            let s = summary(b);
            for r in &s.requires {
                caps.removed.push(CapabilityChange {
                    component: s.name.clone(),
                    capability: r.clone(),
                });
            }
            if !s.requires.is_empty() {
                reduced = true;
            }
            components.removed.push(s);
        }
    }
    // an open island runs in the page's origin (docs/islands.md section 12): one that is added, or one that was sealed and is opened, widens what
    // the program can do, whatever the component says it requires; a sealed island never moves authority
    let open = |f: &IrFile| -> BTreeSet<String> {
        f.islands
            .iter()
            .filter(|i| !i.sealed)
            .map(|i| i.name.clone())
            .collect()
    };
    let (open_b, open_c) = (open(base), open(candidate));
    expanded |= open_c.difference(&open_b).next().is_some();
    reduced |= open_b.difference(&open_c).next().is_some();
    let authority = if expanded {
        Authority::Expanded
    } else if reduced {
        Authority::Reduced
    } else {
        Authority::Unchanged
    };
    caps.added.sort();
    caps.removed.sort();

    let theme = diff_theme(base, candidate);
    let page = diff_page(base, candidate);
    let assets = diff_assets(base, candidate);
    let islands = diff_islands(base, candidate);
    let change_count = components.added.len()
        + components.removed.len()
        + components.changed.iter().map(leaf_count).sum::<usize>()
        + theme.len()
        + page.len()
        + assets.len()
        + islands.len();
    DiffReport {
        schema: SCHEMA,
        version: VERSION,
        base: base_label.to_string(),
        candidate: candidate_label.to_string(),
        changed: change_count > 0,
        authority,
        change_count,
        capabilities: caps,
        components,
        theme,
        page,
        assets,
        islands,
        not_compared: NOT_COMPARED.to_vec(),
    }
}
