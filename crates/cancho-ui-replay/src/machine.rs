//! The component machine: state, the fake clock, the request table and the
//! cancellation groups (docs/replay.md §3-§6). It is the part of the replay that
//! has to agree with the emitted runtime (`__cui_call`, `__cui_dispose`); each
//! rule below cites the emitter's behaviour it mirrors.

use std::collections::BTreeMap;

use cancho_ui_ir::ir::{IrBindingValue, IrComponent, IrExpr, IrScheduleKind, IrStmt, IrViewNode};
use cancho_ui_ir::ir_expr_suspends;
use cancho_ui_syntax::span::Span;

use crate::eval::{Ctx, Fault, Journal, Stop, eval, span_of};
use crate::value::{Effect, Req, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Completed,
    Suspended,
    Dropped,
    /// Every control that fires the event is `disabled` (docs/controls.md section 2): the browser would not have
    /// delivered it, so the handler does not run and nothing changes.
    Disabled,
    /// No control that fires the event is in the page: each is inside an `If` whose condition is false
    /// (docs/view-conditional.md section 4). The browser could not have delivered it; nothing changes.
    Hidden,
    /// The handler faulted and the step said it would (`"fault": "replay-.."`, docs/replay.md): its segment's writes
    /// are undone (docs/semantics.md, "A handler that faults").
    Faulted,
    Ok,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Completed => "completed",
            Outcome::Suspended => "suspended",
            Outcome::Dropped => "dropped",
            Outcome::Disabled => "disabled",
            Outcome::Hidden => "hidden",
            Outcome::Faulted => "faulted",
            Outcome::Ok => "ok",
        }
    }
}

/// What an `await` received.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivered {
    Ok,
    Err,
    Cancelled,
    Dropped,
}

impl Delivered {
    pub fn as_str(self) -> &'static str {
        match self {
            Delivered::Ok => "Ok",
            Delivered::Err => "Err",
            Delivered::Cancelled => "Cancelled",
            Delivered::Dropped => "dropped",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Pending,
    Resolved,
}

/// The statement `pc` points at, or `None` when it points past the end of its block.
fn at<'a>(body: &'a [IrStmt], pc: &[usize]) -> Option<&'a IrStmt> {
    let mut block = body;
    let mut k = 0;
    loop {
        let s = block.get(*pc.get(k)?)?;
        if k + 1 == pc.len() {
            return Some(s);
        }
        let IrStmt::If { then, else_, .. } = s else {
            return None;
        };
        block = if *pc.get(k + 1)? == 0 { then } else { else_ };
        k += 2;
    }
}

/// Move `pc` out of every block it has run off the end of, onto the statement after the `if` that held it, until it points at
/// a statement or past the end of the body.
fn settle(body: &[IrStmt], pc: &mut Vec<usize>) {
    while at(body, pc).is_none() && pc.len() >= 3 {
        pc.truncate(pc.len() - 2);
        if let Some(last) = pc.last_mut() {
            *last += 1;
        }
    }
}

/// A handler waiting on a request.
#[derive(Debug, Clone)]
struct Task {
    handler: usize,
    params: Vec<(String, Value)>,
    /// Where the handler is, as a path through its nested statements (`at`, `settle`): `[i]` is the `i`th statement of the
    /// body; `[i, b, j]` is the `j`th statement of the `then` (`b` = 0) or `else` (`b` = 1) branch of the `if` that is the `i`th.
    pc: Vec<usize>,
    journal: Journal,
}

#[derive(Debug, Clone)]
pub struct Request {
    pub id: u32,
    pub effect: Effect,
    pub args: Vec<Value>,
    /// For `sleep`: the fake-clock time it falls due.
    pub due: Option<f64>,
    /// Superseded by a newer request in its group (`token.cancelled`).
    pub superseded: bool,
    /// Its component was disposed while it was pending (`token.disposed`).
    pub disposed: bool,
    pub status: Status,
    task: Option<Task>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fired {
    pub request: u32,
    pub delivered: Delivered,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Write {
    pub field: String,
    pub span: Span,
}

/// Everything one step did, collected for the report and the expectations.
#[derive(Debug, Default)]
pub struct StepEffects {
    pub issued: Vec<u32>,
    pub fired: Vec<Fired>,
    pub writes: Vec<Write>,
    /// The `every` ticks that fell due in this step (docs/schedules.md section 6).
    pub ticks: Vec<TickRecord>,
    /// The handler the step ran or resumed, for locating `expect` failures.
    pub handler_span: Option<Span>,
}

/// One `every` tick that fell due: when, and whether it ran or was skipped because a run of its event was in flight.
#[derive(Debug, Clone, PartialEq)]
pub struct TickRecord {
    pub event: String,
    pub at: f64,
    pub ran: bool,
    /// The tick that ran when the page became visible again after one fell due while it was hidden.
    pub catch_up: bool,
}

/// The most ticks one `advance` may fire: a mutated or careless trace cannot make replay run (and report) forever.
const MAX_TICKS_PER_ADVANCE: usize = 10_000;

/// The timer of an `every` declaration.
#[derive(Debug, Clone)]
struct TickTimer {
    handler: usize,
    period: f64,
    next: f64,
    span: Span,
}

/// The namespace of the component's `requires Storage("ns")`, or "" when it declares none.
fn storage_ns(comp: &IrComponent) -> &str {
    comp.requires
        .iter()
        .filter(|r| r.name == "Storage")
        .find_map(|r| r.bounds.first())
        .map(String::as_str)
        .unwrap_or("")
}

pub struct Machine<'a> {
    comp: &'a IrComponent,
    pub state: BTreeMap<String, Value>,
    pub clock: f64,
    /// The `Storage` store (docs/adapters.md): key to value, seeded by the trace, written by `storage.set`.
    pub storage: BTreeMap<String, String>,
    /// The quota of the persisted store and the values it refused (see `Ctx`), docs/adapters.md section 20.
    pub storage_quota: Option<u64>,
    pub storage_memory: BTreeMap<String, String>,
    /// The names of the credentials the trace says the page supplied (docs/adapters.md section 18).
    pub credentials: Vec<String>,
    pub requests: Vec<Request>,
    inflight: BTreeMap<&'static str, u32>,
    pub disposed: bool,
    ticks: Vec<TickTimer>,
    started: bool,
    /// Is the page visible? While it is not, no `every` tick runs (docs/schedules.md section 11).
    pub visible: bool,
}

impl<'a> Machine<'a> {
    /// Initialise state from the declared initial values, in declaration order.
    pub fn new(comp: &'a IrComponent, clock: f64) -> Result<Machine<'a>, Fault> {
        let mut state: BTreeMap<String, Value> = BTreeMap::new();
        let mut no_storage: BTreeMap<String, String> = BTreeMap::new();
        let mut no_memory: BTreeMap<String, String> = BTreeMap::new();
        for field in &comp.state {
            let mut journal = Journal::default();
            let value = {
                let mut ctx = Ctx {
                    state: &state,
                    clock,
                    storage: &mut no_storage,
                    storage_ns: "",
                    storage_quota: None,
                    storage_memory: &mut no_memory,
                    journal: &mut journal,
                    locals: Vec::new(),
                    init: true,
                };
                match eval(&mut ctx, &field.initial) {
                    Ok(v) => v,
                    Err(Stop::Fault(f)) => return Err(f),
                    Err(Stop::Suspend(_)) => {
                        return Err(Fault {
                            rule: "replay-unsupported",
                            message: "an `await` in a state initial value".into(),
                            span: span_of(&field.initial),
                        });
                    }
                }
            };
            state.insert(field.name.clone(), value);
        }
        Ok(Machine {
            comp,
            state,
            clock,
            storage: BTreeMap::new(),
            storage_quota: None,
            storage_memory: BTreeMap::new(),
            credentials: Vec::new(),
            requests: Vec::new(),
            inflight: BTreeMap::new(),
            disposed: false,
            ticks: comp
                .schedules
                .iter()
                .filter_map(|s| match s.kind {
                    IrScheduleKind::Every(ms) => comp
                        .events
                        .iter()
                        .position(|e| e.name == s.event)
                        .map(|handler| TickTimer {
                            handler,
                            period: f64::from(ms),
                            next: clock + f64::from(ms),
                            span: s.span,
                        }),
                    IrScheduleKind::Start => None,
                })
                .collect(),
            started: false,
            visible: true,
        })
    }

    /// Run the `start` events, once, in the order written (docs/schedules.md section 6): the component has mounted.
    pub fn start(&mut self, fx: &mut StepEffects) -> Result<(), Fault> {
        if self.started {
            return Ok(());
        }
        self.started = true;
        let comp = self.comp;
        for s in &comp.schedules {
            if s.kind == IrScheduleKind::Start
                && let Some(h) = comp.events.iter().position(|e| e.name == s.event)
            {
                self.dispatch(h, Vec::new(), fx)?;
            }
        }
        Ok(())
    }

    /// Is a run of handler `h` suspended on a request that is still current? A tick is skipped then
    /// (docs/schedules.md section 4, rule 3).
    fn in_flight(&self, h: usize) -> bool {
        self.requests.iter().any(|r| {
            r.status == Status::Pending
                && !r.superseded
                && !r.disposed
                && r.task.as_ref().is_some_and(|t| t.handler == h)
        })
    }

    pub fn handler_index(&self, event: &str) -> Option<usize> {
        self.comp.events.iter().position(|e| e.name == event)
    }

    pub fn handler_params(&self, handler: usize) -> usize {
        self.comp.events[handler].params.len()
    }

    pub fn request(&self, id: u32) -> Option<&Request> {
        id.checked_sub(1)
            .and_then(|i| self.requests.get(i as usize))
    }

    pub fn pending(&self) -> Vec<u32> {
        self.requests
            .iter()
            .filter(|r| r.status == Status::Pending)
            .map(|r| r.id)
            .collect()
    }

    /// Deliver an event: run the handler to its end or its first unanswered
    /// `await`. The caller has checked the handler exists and the arity.
    pub fn dispatch(
        &mut self,
        handler: usize,
        params: Vec<Value>,
        fx: &mut StepEffects,
    ) -> Result<Outcome, Fault> {
        if let Some(refused) = self.event_refused(&self.comp.events[handler].name) {
            return Ok(refused);
        }
        let names = &self.comp.events[handler].params;
        let task = Task {
            handler,
            params: names.iter().cloned().zip(params).collect(),
            pc: vec![0],
            journal: Journal::default(),
        };
        self.run(task, fx, false)
    }

    /// Is the control under an `If { when: cond }` in the page right now? The condition is read from the state; one that cannot
    /// be evaluated here (it reads a `For` binding, or faults) counts as true (docs/view-conditional.md section 4).
    pub(crate) fn condition_holds(&self, cond: &IrExpr) -> bool {
        let mut journal = Journal::default();
        let mut no_storage = BTreeMap::new();
        let mut no_memory = BTreeMap::new();
        let mut ctx = Ctx {
            state: &self.state,
            clock: self.clock,
            storage: &mut no_storage,
            storage_ns: "",
            storage_quota: None,
            storage_memory: &mut no_memory,
            journal: &mut journal,
            locals: Vec::new(),
            init: false,
        };
        !matches!(eval(&mut ctx, cond), Ok(Value::Bool(false)))
    }

    /// The most characters (UTF-16 code units) any way of firing `event` with a typed value allows, when every control in the
    /// page that fires it is a `TextField` with a `maxlength:`; `None` when some control without a limit (or a button) could give
    /// it a value of any length, or no control fires it (docs/controls.md section 6).
    pub fn input_limit(&self, event: &str) -> Option<usize> {
        #[derive(Default)]
        struct Fields {
            limits: Vec<Option<usize>>,
            other: bool,
        }
        fn walk(m: &Machine<'_>, node: &IrViewNode, event: &str, f: &mut Fields) {
            match node {
                IrViewNode::Element {
                    kind,
                    bindings,
                    children,
                    ..
                } => {
                    let fires = bindings.iter().any(|b| {
                        b.key == "on"
                            && match &b.value {
                                IrBindingValue::EventRef(e) => e == event,
                                IrBindingValue::EventCall { name, .. } => name == event,
                                _ => false,
                            }
                    });
                    if fires {
                        if matches!(kind, cancho_ui_ir::ir::IrElementKind::TextField) {
                            let limit = bindings.iter().find_map(|b| match (&*b.key, &b.value) {
                                ("maxlength", IrBindingValue::Const { expr, .. }) => match **expr {
                                    IrExpr::IntLit { value, .. } => usize::try_from(value).ok(),
                                    _ => None,
                                },
                                _ => None,
                            });
                            f.limits.push(limit);
                        } else {
                            f.other = true;
                        }
                    }
                    for c in children {
                        walk(m, c, event, f);
                    }
                }
                IrViewNode::For { children, .. } => {
                    for c in children {
                        walk(m, c, event, f);
                    }
                }
                IrViewNode::If { cond, children, .. } => {
                    if m.condition_holds(cond) {
                        for c in children {
                            walk(m, c, event, f);
                        }
                    }
                }
                IrViewNode::Text { .. } => {}
            }
        }
        let mut f = Fields::default();
        walk(self, &self.comp.view.body, event, &mut f);
        if f.other || f.limits.is_empty() || f.limits.iter().any(Option::is_none) {
            return None;
        }
        f.limits.into_iter().flatten().max()
    }

    /// Would the browser deliver `event` right now (docs/controls.md section 2)? The ways to fire it are the controls with
    /// `on: event` (a click or an input: open when the control is enabled) and the fields with `submit: event` (Enter:
    /// open when the field is enabled **and** every `on:` control of the event is enabled, because the emitted guard
    /// `__submit_blocked` treats Enter as the same act as the button). An event no control fires (a schedule's) is
    /// never refused. Only `true`, `false` and a `Bool` state field can be `disabled:`. A control inside an `If` whose
    /// condition is false is not in the page: if that is every control of the event, it is `Hidden`
    /// (docs/view-conditional.md section 4). The condition is read from the state as it is now; a condition that cannot
    /// be evaluated here (it reads a `For` binding, which this walk does not iterate, or faults) counts as true.
    fn event_refused(&self, event: &str) -> Option<Outcome> {
        #[derive(Default)]
        struct Paths {
            on: Vec<bool>,
            submit: Vec<bool>,
            hidden: usize,
        }
        /// Does this node, or anything under it, fire `event`?
        fn fires_any(node: &IrViewNode, event: &str) -> usize {
            match node {
                IrViewNode::Element {
                    bindings, children, ..
                } => {
                    let own = bindings
                        .iter()
                        .filter(|b| {
                            (b.key == "on" || b.key == "submit")
                                && match &b.value {
                                    IrBindingValue::EventRef(e) => e == event,
                                    IrBindingValue::EventCall { name, .. } => name == event,
                                    _ => false,
                                }
                        })
                        .count();
                    own + children.iter().map(|c| fires_any(c, event)).sum::<usize>()
                }
                IrViewNode::For { children, .. } | IrViewNode::If { children, .. } => {
                    children.iter().map(|c| fires_any(c, event)).sum()
                }
                IrViewNode::Text { .. } => 0,
            }
        }
        fn walk(m: &Machine<'_>, node: &IrViewNode, event: &str, p: &mut Paths) {
            match node {
                IrViewNode::Element {
                    bindings, children, ..
                } => {
                    let fires = |key: &str| {
                        bindings.iter().any(|b| {
                            b.key == key
                                && match &b.value {
                                    IrBindingValue::EventRef(e) => e == event,
                                    // `on: Pick(m)` fires `Pick` like `on: Pick` does (docs/row-actions.md).
                                    IrBindingValue::EventCall { name, .. } => name == event,
                                    _ => false,
                                }
                        })
                    };
                    let off = bindings.iter().any(|b| {
                        b.key == "disabled"
                            && match &b.value {
                                IrBindingValue::Const { expr, .. } => {
                                    matches!(**expr, IrExpr::BoolLit { value: true, .. })
                                }
                                IrBindingValue::StateRef(name) => {
                                    matches!(m.state.get(name), Some(Value::Bool(true)))
                                }
                                _ => false,
                            }
                    });
                    if fires("on") {
                        p.on.push(!off);
                    }
                    if fires("submit") {
                        p.submit.push(!off);
                    }
                    for c in children {
                        walk(m, c, event, p);
                    }
                }
                IrViewNode::For { children, .. } => {
                    for c in children {
                        walk(m, c, event, p);
                    }
                }
                IrViewNode::If { cond, children, .. } => {
                    if m.condition_holds(cond) {
                        for c in children {
                            walk(m, c, event, p);
                        }
                    } else {
                        p.hidden += children.iter().map(|c| fires_any(c, event)).sum::<usize>();
                    }
                }
                IrViewNode::Text { .. } => {}
            }
        }
        let mut p = Paths::default();
        walk(self, &self.comp.view.body, event, &mut p);
        if p.on.is_empty() && p.submit.is_empty() {
            return (p.hidden > 0).then_some(Outcome::Hidden);
        }
        let all_on_enabled = p.on.iter().all(|e| *e);
        let open = p.on.iter().any(|e| *e) || (all_on_enabled && p.submit.iter().any(|e| *e));
        (!open).then_some(Outcome::Disabled)
    }

    /// The scripted network answers request `id` (`answer` is an `Ok` or `Err`).
    /// The caller has checked the request is a pending `net.search`.
    pub fn resolve(
        &mut self,
        id: u32,
        answer: Value,
        fx: &mut StepEffects,
    ) -> Result<Outcome, Fault> {
        self.deliver(id, answer, fx)
    }

    /// Move the fake clock forward; due `sleep`s fire in `(due, id)` order, the
    /// clock reading each one's due time as it fires (docs/replay.md §5).
    pub fn advance(&mut self, ms: f64, fx: &mut StepEffects) -> Result<Outcome, Fault> {
        let target = self.clock + ms;
        let mut fired = 0usize;
        loop {
            let sleep = self
                .requests
                .iter()
                .filter(|r| r.status == Status::Pending && r.effect == Effect::Sleep)
                .filter_map(|r| r.due.filter(|d| *d <= target).map(|d| (d, r.id)))
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            // A tick due at the same instant as a `sleep` runs first, and ticks due together run in the order the
            // `every` lines are written (docs/schedules.md section 4, rule 6).
            let tick = if self.disposed || !self.visible {
                None
            } else {
                self.ticks
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.next <= target)
                    .min_by(|a, b| a.1.next.total_cmp(&b.1.next).then(a.0.cmp(&b.0)))
                    .map(|(i, t)| (t.next, i))
            };
            // A sleep goes first when no tick is due, or when it is due strictly before the next tick. Choosing it here, then
            // matching on the tick alone, leaves no combination of (sleep, tick) without an arm.
            let first_sleep = match (sleep, tick) {
                (Some((due, id)), None) => Some((due, id)),
                (Some((due, id)), Some((at, _))) if due < at => Some((due, id)),
                _ => None,
            };
            if let Some((due, id)) = first_sleep {
                if due > self.clock {
                    self.clock = due;
                }
                self.deliver(id, Value::Ok(Box::new(Value::Unit)), fx)?;
                continue;
            }
            match tick {
                None => break,
                Some((at, i)) => {
                    if at > self.clock {
                        self.clock = at;
                    }
                    fired += 1;
                    if fired > MAX_TICKS_PER_ADVANCE {
                        return Err(Fault {
                            rule: "replay-tick-limit",
                            message: format!(
                                "one `advance` fires more than {MAX_TICKS_PER_ADVANCE} ticks; split it into shorter steps or lengthen the period"
                            ),
                            span: self.ticks[i].span,
                        });
                    }
                    let handler = self.ticks[i].handler;
                    let ran = !self.in_flight(handler);
                    self.ticks[i].next += self.ticks[i].period;
                    fx.ticks.push(TickRecord {
                        event: self.comp.events[handler].name.clone(),
                        at,
                        ran,
                        catch_up: false,
                    });
                    if ran {
                        self.dispatch(handler, Vec::new(), fx)?;
                    }
                }
            }
        }
        self.clock = target;
        Ok(Outcome::Ok)
    }

    /// The page is hidden: no tick runs until it is shown again (docs/schedules.md section 11, rule 8).
    pub fn hide(&mut self) -> Outcome {
        self.visible = false;
        Outcome::Ok
    }

    /// The page is visible again: each `every` whose tick fell due while it was hidden runs once now, in the order
    /// written (unless a run of its event is in flight), and its next tick is one period from now; the others keep the
    /// time they had left (rule 9).
    pub fn show(&mut self, fx: &mut StepEffects) -> Result<Outcome, Fault> {
        self.visible = true;
        if self.disposed {
            return Ok(Outcome::Ok);
        }
        for i in 0..self.ticks.len() {
            if self.ticks[i].next <= self.clock {
                let handler = self.ticks[i].handler;
                let ran = !self.in_flight(handler);
                self.ticks[i].next = self.clock + self.ticks[i].period;
                fx.ticks.push(TickRecord {
                    event: self.comp.events[handler].name.clone(),
                    at: self.clock,
                    ran,
                    catch_up: true,
                });
                if ran {
                    self.dispatch(handler, Vec::new(), fx)?;
                }
            }
        }
        Ok(Outcome::Ok)
    }

    /// The host unmounts the component (`__cui_dispose`): the current pending
    /// request of each group is marked disposed and the table emptied.
    pub fn dispose(&mut self) -> Outcome {
        let current: Vec<u32> = self.inflight.values().copied().collect();
        for id in current {
            if let Some(r) = self.requests.get_mut(id as usize - 1) {
                r.disposed = true;
            }
        }
        self.inflight.clear();
        self.disposed = true;
        Outcome::Ok
    }

    /// A request's underlying effect completed with `answer`. Mirrors
    /// `__cui_call`'s `.then`: superseded first (drop), then disposed
    /// (`Cancelled`), else clear the slot and deliver.
    fn deliver(&mut self, id: u32, answer: Value, fx: &mut StepEffects) -> Result<Outcome, Fault> {
        let idx = id as usize - 1;
        let (mut task, superseded, disposed, group) = {
            let r = &mut self.requests[idx];
            // A request that no longer holds its task has nothing left to deliver (it was settled already).
            let Some(task) = r.task.take() else {
                return Ok(Outcome::Ok);
            };
            r.status = Status::Resolved;
            (task, r.superseded, r.disposed, r.effect.group())
        };
        if superseded {
            fx.fired.push(Fired {
                request: id,
                delivered: Delivered::Dropped,
                outcome: Outcome::Dropped,
            });
            return Ok(Outcome::Dropped);
        }
        let (delivered, value) = if disposed {
            (Delivered::Cancelled, Value::Cancelled)
        } else {
            if self.inflight.get(group) == Some(&id) {
                self.inflight.remove(group);
            }
            let kind = match answer {
                Value::Err(_) => Delivered::Err,
                _ => Delivered::Ok,
            };
            (kind, answer)
        };
        task.journal.deliver(value);
        let outcome = self.run(task, fx, true)?;
        fx.fired.push(Fired {
            request: id,
            delivered,
            outcome,
        });
        Ok(outcome)
    }

    /// Run a handler from `task.pc` until it completes, suspends or faults.
    ///
    /// A handler is atomic between its commit points (docs/semantics.md, "A handler that faults"): the start of the
    /// handler, and the end of each statement that awaited. A fault undoes the writes made since the last commit point
    /// and the report forgets them. `resumed` is true when the run continues an awaiting statement: the statements
    /// before it were committed when the handler suspended, and the statement itself writes only when it completes, so
    /// a fault in it has nothing to undo.
    fn run(
        &mut self,
        mut task: Task,
        fx: &mut StepEffects,
        resumed: bool,
    ) -> Result<Outcome, Fault> {
        let comp = self.comp;
        let handler = &comp.events[task.handler];
        fx.handler_span = Some(handler.span);
        let mut commit: Option<(BTreeMap<String, Value>, usize)> = if resumed {
            None
        } else {
            Some((self.state.clone(), fx.writes.len()))
        };
        loop {
            settle(&handler.body, &mut task.pc);
            let Some(stmt) = at(&handler.body, &task.pc) else {
                return Ok(Outcome::Completed);
            };
            task.journal.rewind();
            let result = {
                let mut ctx = Ctx {
                    state: &self.state,
                    clock: self.clock,
                    storage: &mut self.storage,
                    storage_ns: storage_ns(comp),
                    storage_quota: self.storage_quota,
                    storage_memory: &mut self.storage_memory,
                    journal: &mut task.journal,
                    locals: task.params.clone(),
                    init: false,
                };
                match stmt {
                    IrStmt::Assign { value, .. } | IrStmt::Expr { expr: value, .. } => {
                        eval(&mut ctx, value)
                    }
                    IrStmt::If { cond, span, .. } => eval(&mut ctx, cond).and_then(|v| match v {
                        Value::Bool(_) => Ok(v),
                        other => Err(Stop::Fault(Fault {
                            rule: "replay-type-error",
                            message: format!(
                                "the condition of an `if` statement needs a bool, got {}",
                                crate::eval::describe(&other)
                            ),
                            span: *span,
                        })),
                    }),
                }
            };
            match result {
                Ok(v) => {
                    if let IrStmt::If { .. } = stmt {
                        // The branch is chosen here, once: a handler that resumes inside it does not read the condition again.
                        let branch = usize::from(v != Value::Bool(true));
                        task.pc.push(branch);
                        task.pc.push(0);
                        task.journal.clear();
                        continue;
                    }
                    if let IrStmt::Assign { target, span, .. } = stmt {
                        if !self.state.contains_key(target) {
                            return Err(self.undo(
                                commit,
                                fx,
                                Fault {
                                    rule: "replay-unknown-identifier",
                                    message: format!(
                                        "assignment to `{target}`, which is not a state field"
                                    ),
                                    span: *span,
                                },
                            ));
                        }
                        self.state.insert(target.clone(), v);
                        fx.writes.push(Write {
                            field: target.clone(),
                            span: *span,
                        });
                    }
                    if let Some(last) = task.pc.last_mut() {
                        *last += 1;
                    }
                    task.journal.clear();
                    // A statement that awaited ends a segment: what it wrote is committed.
                    let awaited = match stmt {
                        IrStmt::Assign { value, .. } | IrStmt::Expr { expr: value, .. } => {
                            ir_expr_suspends(value)
                        }
                        IrStmt::If { .. } => false,
                    };
                    if awaited {
                        commit = Some((self.state.clone(), fx.writes.len()));
                    }
                }
                Err(Stop::Suspend(req)) => {
                    self.issue(req, task, fx);
                    return Ok(Outcome::Suspended);
                }
                Err(Stop::Fault(f)) => return Err(self.undo(commit, fx, f)),
            }
        }
    }

    /// Undo the writes of the segment that faulted: restore the state of the last commit point and drop the writes
    /// the step's report lists since. Effects already performed (`storage.set`, a request already issued) stay.
    fn undo(
        &mut self,
        commit: Option<(BTreeMap<String, Value>, usize)>,
        fx: &mut StepEffects,
        fault: Fault,
    ) -> Fault {
        if let Some((state, writes)) = commit {
            self.state = state;
            fx.writes.truncate(writes);
        }
        fault
    }

    /// Start a request: supersede the group's pending one, then register it
    /// (`__cui_call`).
    fn issue(&mut self, req: Req, task: Task, fx: &mut StepEffects) {
        let group = req.effect.group();
        if let Some(prev) = self.inflight.get(group).copied()
            && let Some(p) = self.requests.get_mut(prev as usize - 1)
        {
            p.superseded = true;
        }
        let id = self.requests.len() as u32 + 1;
        self.inflight.insert(group, id);
        let due = match (req.effect, req.args.first()) {
            (Effect::Sleep, Some(Value::Num(ms))) => Some(self.clock + ms),
            _ => None,
        };
        self.requests.push(Request {
            id,
            effect: req.effect,
            args: req.args,
            due,
            superseded: false,
            disposed: false,
            status: Status::Pending,
            task: Some(task),
        });
        fx.issued.push(id);
    }
}
