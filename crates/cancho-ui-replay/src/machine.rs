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

/// A handler waiting on a request.
#[derive(Debug, Clone)]
struct Task {
    handler: usize,
    params: Vec<(String, Value)>,
    stmt: usize,
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
        if self.event_disabled(&self.comp.events[handler].name) {
            return Ok(Outcome::Disabled);
        }
        let names = &self.comp.events[handler].params;
        let task = Task {
            handler,
            params: names.iter().cloned().zip(params).collect(),
            stmt: 0,
            journal: Journal::default(),
        };
        self.run(task, fx, false)
    }

    /// Would the browser deliver `event` right now (docs/controls.md section 2)? The ways to fire it are the controls with
    /// `on: event` (a click or an input: open when the control is enabled) and the fields with `submit: event` (Enter:
    /// open when the field is enabled **and** every `on:` control of the event is enabled, because the emitted guard
    /// `__submit_blocked` treats Enter as the same act as the button). An event no control fires (a schedule's) is
    /// never refused. Only `true`, `false` and a `Bool` state field can be `disabled:`.
    fn event_disabled(&self, event: &str) -> bool {
        #[derive(Default)]
        struct Paths {
            on: Vec<bool>,
            submit: Vec<bool>,
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
                IrViewNode::Text { .. } => {}
            }
        }
        let mut p = Paths::default();
        walk(self, &self.comp.view.body, event, &mut p);
        if p.on.is_empty() && p.submit.is_empty() {
            return false;
        }
        let all_on_enabled = p.on.iter().all(|e| *e);
        let open = p.on.iter().any(|e| *e) || (all_on_enabled && p.submit.iter().any(|e| *e));
        !open
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

    /// Run a handler from `task.stmt` until it completes, suspends or faults.
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
            let Some(stmt) = handler.body.get(task.stmt) else {
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
                }
            };
            match result {
                Ok(v) => {
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
                    task.stmt += 1;
                    task.journal.clear();
                    // A statement that awaited ends a segment: what it wrote is committed.
                    let awaited = match stmt {
                        IrStmt::Assign { value, .. } | IrStmt::Expr { expr: value, .. } => {
                            ir_expr_suspends(value)
                        }
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
