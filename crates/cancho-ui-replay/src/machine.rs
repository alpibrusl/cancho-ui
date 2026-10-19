//! The component machine: state, the fake clock, the request table and the
//! cancellation groups (docs/replay.md §3-§6). It is the part of the replay that
//! has to agree with the emitted runtime (`__cui_call`, `__cui_dispose`); each
//! rule below cites the emitter's behaviour it mirrors.

use std::collections::BTreeMap;

use cancho_ui_ir::ir::{IrComponent, IrStmt};
use cancho_ui_syntax::span::Span;

use crate::eval::{Ctx, Fault, Journal, Stop, eval, span_of};
use crate::value::{Effect, Req, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Completed,
    Suspended,
    Dropped,
    Ok,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Completed => "completed",
            Outcome::Suspended => "suspended",
            Outcome::Dropped => "dropped",
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
    /// The handler the step ran or resumed, for locating `expect` failures.
    pub handler_span: Option<Span>,
}

pub struct Machine<'a> {
    comp: &'a IrComponent,
    pub state: BTreeMap<String, Value>,
    pub clock: f64,
    /// The `Storage` store (docs/adapters.md): key to value, seeded by the trace, written by `storage.set`.
    pub storage: BTreeMap<String, String>,
    pub requests: Vec<Request>,
    inflight: BTreeMap<&'static str, u32>,
    pub disposed: bool,
}

impl<'a> Machine<'a> {
    /// Initialise state from the declared initial values, in declaration order.
    pub fn new(comp: &'a IrComponent, clock: f64) -> Result<Machine<'a>, Fault> {
        let mut state: BTreeMap<String, Value> = BTreeMap::new();
        let mut no_storage: BTreeMap<String, String> = BTreeMap::new();
        for field in &comp.state {
            let mut journal = Journal::default();
            let value = {
                let mut ctx = Ctx {
                    state: &state,
                    clock,
                    storage: &mut no_storage,
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
            requests: Vec::new(),
            inflight: BTreeMap::new(),
            disposed: false,
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
        let names = &self.comp.events[handler].params;
        let task = Task {
            handler,
            params: names.iter().cloned().zip(params).collect(),
            stmt: 0,
            journal: Journal::default(),
        };
        self.run(task, fx)
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
        loop {
            let next = self
                .requests
                .iter()
                .filter(|r| r.status == Status::Pending && r.effect == Effect::Sleep)
                .filter_map(|r| r.due.filter(|d| *d <= target).map(|d| (d, r.id)))
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let Some((due, id)) = next else { break };
            if due > self.clock {
                self.clock = due;
            }
            self.deliver(id, Value::Ok(Box::new(Value::Unit)), fx)?;
        }
        self.clock = target;
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
            r.status = Status::Resolved;
            (
                r.task.take().expect("a pending request holds its task"),
                r.superseded,
                r.disposed,
                r.effect.group(),
            )
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
        let outcome = self.run(task, fx)?;
        fx.fired.push(Fired {
            request: id,
            delivered,
            outcome,
        });
        Ok(outcome)
    }

    fn run(&mut self, mut task: Task, fx: &mut StepEffects) -> Result<Outcome, Fault> {
        let comp = self.comp;
        let handler = &comp.events[task.handler];
        fx.handler_span = Some(handler.span);
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
                            return Err(Fault {
                                rule: "replay-unknown-identifier",
                                message: format!(
                                    "assignment to `{target}`, which is not a state field"
                                ),
                                span: *span,
                            });
                        }
                        self.state.insert(target.clone(), v);
                        fx.writes.push(Write {
                            field: target.clone(),
                            span: *span,
                        });
                    }
                    task.stmt += 1;
                    task.journal.clear();
                }
                Err(Stop::Suspend(req)) => {
                    self.issue(req, task, fx);
                    return Ok(Outcome::Suspended);
                }
                Err(Stop::Fault(f)) => return Err(f),
            }
        }
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
