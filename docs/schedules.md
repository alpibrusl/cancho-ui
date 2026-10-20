# Schedules: `start` and `every`

> **Status: designed before built.** Written 2026-10-20 before any code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1),
> after `examples/plant` (adapters.md section 14) showed what is missing: a console that watches a gateway refreshes only
> when someone presses a button, because the language has no timer and no start event. A claim found false while building
> is corrected here, in place, and listed in section 10. Guarantee tier of the replay semantics: **tested**; of the browser
> behaviour: **observed in Chromium**, not guaranteed (section 7).

## 1. The decision

Two declarations in a component, next to `state`, `requires`, `on` and `view`:

```
start Refresh;          // run the event Refresh once, when the component mounts
every 5000 Refresh;     // run the event Refresh every 5000 ms while the component is mounted
```

A schedule **names an event the component already handles**. It is not a new kind of handler and has no body: what runs
is the ordinary `on Refresh { ... }`, the same one a button fires. So everything the language already says about a handler
(`await`, supersede, `Cancelled`, replay) holds for a scheduled run, and a reader finds the whole behaviour in one place.

The period is an **integer literal in milliseconds**, so the rate is visible in the source, in `inspect`, in the IR and in a
semantic diff, where a change from `every 5000` to `every 100` is a change of behaviour a reviewer must see.

## 2. Why this shape, and not another

| considered | why not |
|---|---|
| an `on Every(5000) { ... }` handler head | a second kind of handler, with its own body semantics; the rate hides inside the handler's header |
| a `Timer` capability (`requires Timer;`) | `sleep` needs no capability (the runtime scheduler is its capability, `semantics.md` Q6). A timer reaches nothing outside the page: what it triggers is already declared, because the event it runs names its own `requires`. A periodic `net.get` is visible as `Network`, and the period is visible as the schedule |
| a loop in a handler (`sleep` then recurse) | the language has no loops and no recursion; adding one for this is the larger change |
| a lifecycle event `on Mount` | covers `start` only, and puts the rate question back to a handler that re-arms itself |
| a general `setInterval` builtin | an effect that returns a handle needs handles, cancellation and ownership in the language |

## 3. Syntax and rules

```
schedule_decl := "start" identifier ";" | "every" int_literal identifier ";"
```

`start` and `every` are read as keywords only at the start of a component member; everywhere else they are identifiers
(a state field called `start` is unaffected). Refusals, each a stable diagnostic code:

| code | when |
|---|---|
| `schedule-event-unknown` | the event has no `on` handler in this component |
| `schedule-event-params` | the handler takes parameters: a scheduled run has none to give |
| `schedule-period-invalid` | the period is not an integer literal from 100 to 86,400,000 (a tenth of a second to a day) |
| `schedule-duplicate` | the same `start` event twice, or the same `every` event twice (two periods for one event would be two things racing) |

A component may have any number of schedules; the cost of many is the author's.

## 4. Semantics

1. **`start`** runs its event once, **after the first render** of the component, in the order the `start` lines are
   written. The run is synchronous up to its first `await`, like a click.
2. **`every N`** runs its event first at *mount + N* ms and then every N ms. It is a timer of the page, not of the
   wall clock: nothing is aligned to a time of day.
3. **A run never overlaps itself.** A tick that falls due while a run of the same event is still in flight (suspended on an
   `await`) is **skipped**, not queued and not started on top: starting a request in a group supersedes the pending one
   (`semantics.md` Q4), so a poll slower than its period would otherwise cancel itself forever. "In flight" counts a run
   started by a click or by `start` as well, not only by a tick.
4. **Disposal stops them.** When a component is disposed (`semantics.md` Q1, Tier 17), its `every` timers are cleared with
   its pending requests, and no tick runs afterwards. The page's root component is never disposed.
5. **No capability.** A schedule asks for no authority and adds none to the report; the events it runs carry their own.
6. **Ties.** Two ticks due at the same instant run in the order the `every` lines are written. In replay (section 6) a tick
   due at the same instant as a `sleep` runs first. The browser promises no order between timers due at the same
   millisecond, so a program must not depend on one.

## 5. IR, inspect and diff

* The IR component gains `schedules: [{kind: "start"|"every", event, period_ms (every only), span}]`, between `requires` and
  `events`. It is part of `inspect ir` and of the component's stable identity.
* `inspect component` lists them.
* `diff` reports a schedule added, removed, or with a changed period, under the component, as a **behavioural** change
  (`docs/semantic-diff.md`).

## 6. Replay

The fake clock already moves with `{"advance": ms}` and fires `sleep`s (`replay.md` section 5). It now fires ticks too:

* The `start` events of the replayed component run **before the first step**, in order; what they issue (request ids,
  writes) is reported with the first step, so step numbering and every existing trace are unchanged.
* `advance` fires, in time order, the `sleep`s that fall due and the ticks that fall due; the clock reads each one's due
  time as it runs. A tick whose event has a run in flight is skipped (rule 3) and says so.
* A step reports `ticks: [{"event": "Refresh", "at": 5000, "ran": true|false}]`, and `expect.ticks` can assert it (subset
  rules of `replay.md` section 2.3).
* `dispose` stops further ticks. `hide` and `show` steps pause and resume them (section 11.3).
* The first tick is at the clock of the trace header (`clock`, default 0) plus the period.

## 7. The browser

`start`: after the first `__render()`, `__on_E()` is called. `every N`: a timer with a guard on a per-event in-flight
count that every run of that event (clicks included) increments and decrements around its body; `__cui_dispose` clears the
timers. *(Section 11.4: the first version was a `setInterval`; it is a `setTimeout` chain now, paused while the page is hidden.)* This needs the component to return a dispose handle, which it already does when it awaits or holds
children, and now also when it has a schedule.

What the browser does not promise, and the page does not hide: a tab in the background has its timers throttled (about
once a second, more in a long-idle tab), a laptop that sleeps stops them, and `setInterval` drifts. Replay's time is
exact; the page's is not. A schedule is "about every N ms while the page can run it".

## 8. What it does not do

Wall-clock times (cron), a backoff after failures, a schedule that starts or stops from state, jitter, a period computed at run time, and a schedule on a component instance created by a parent with
properties (it follows the component, so it runs once per instance).

## 9. Checks planned

* Unit: the parser (both forms, the keywords as identifiers elsewhere), each refusal code, the IR serialisation.
* Replay: `start` before step 1; ticks at the right times; skipped while in flight; no overlap with a `sleep` of the same
  instant; `dispose` stops them; existing traces unchanged.
* Browser (Chromium, `page.clock`): a tick every period; skipped while a slow request is in flight; none after the
  component is disposed; throttling is not tested.
* Diff: a period change is reported.
* The plant console polls: `start Refresh; every 5000 Refresh;`.

## 10. Corrections

- Built 2026-10-20. The design held except two details found by the tests: the replay step report lists a tick as
  `{event, at, ran}` and `expect.ticks` is a subset check, so a trace may omit `ran`; and ticks due at one instant fire
  in the order the `every` lines are written (not alphabetical by event), as section 6 already said.
- Section 8 said a hidden page is not paused; section 11 pauses it. Section 7 said `setInterval`; the emitted code is a `setTimeout`
  chain since section 11 (same period, no drift, no burst after a throttled tab). `mobile-backend.md` said `pagehide` maps to
  disposal; it does not (11.2, rule 12).

## 11. Hidden pages (written 2026-10-20, built the same day)

Section 8 said a schedule does not pause while the page is hidden, and section 7 said a background tab is throttled. A
phone locks, a tab goes to the back, and the console keeps polling a gateway nobody is looking at. This section is the
rule that stops it. It adds no syntax and no IR: the behaviour is fixed, like the skip rule.

### 11.1 The decision

A page is **active** when `document.visibilityState` is not `"hidden"`. While it is not active, no `every` tick runs. When
it becomes active again, a schedule whose tick came due meanwhile runs **once, at once** (still subject to the skip rule),
and its next tick is one period after that instant; a schedule not yet due carries on with the time it had left. Missed
ticks are not replayed, for the reason ticks are not queued (rule 3).

### 11.2 Rules

7. **Active** is `visibilityState !== "hidden"` at mount and after each `visibilitychange`. `pagehide` makes the page
   inactive and a `pageshow` with `persisted` set (a page restored from the back-forward cache) makes it active again if it
   is visible. A repeated change to the state the page is already in is ignored.
8. **Inactive: no tick.** A tick that was due while inactive does not run then.
9. **Becoming active.** For each `every` in the order written: if its next tick time has passed, run its event once (rule 3
   applies: not if a run is in flight) and set its next tick to the current time plus the period; otherwise keep its next
   tick time. A burst of missed ticks is one catch-up tick.
10. **Nothing else pauses.** `start` has already run; requests in flight complete and write state; `sleep` timers fire (the
    browser may throttle them). Being inactive is not disposal: no request is cancelled and no state is dropped.
11. **Opened hidden.** `start` runs at mount; an `every` timer is not armed, and rule 9 applies the first time the page is
    visible, with the first tick due one period after mount.
12. **`pagehide` is not disposal.** It used to be written as "`pagehide` maps to disposal" (`mobile-backend.md`, corrected
    below): a page that goes into the back-forward cache comes back, and a disposed component cannot. Disposal stays what it
    was: a parent removing the component (`semantics.md` Q1, Tier 17). A page that really unloads loses its timers with it.

A component with no `every` registers no listener and emits nothing for any of this.

### 11.3 Replay

Two steps: `{"hide": true}` and `{"show": true}`. A step that repeats the state the page is in is `trace-bad-step`, so a trace
cannot say something the browser would not. While hidden, `advance` fires `sleep`s and no ticks. A `show` step reports the
catch-up ticks it ran in `ticks`, each with `"catch_up": true` and `at` the clock. The clock does not move on `hide`
or `show`.

### 11.4 The browser

Each `every` becomes a descriptor `{ms, next, run, o}` and a `setTimeout` chain that reschedules from `next`, not from the
time it ran (so it does not drift, as `setInterval` did not), and that sets `next` to now plus a period if the browser ran it
more than a period late (so a throttled tab does not fire a burst). `visibilitychange`, `pagehide` and `pageshow` are
listened to only by a component that has an `every`, and `__cui_dispose` removes the listeners with the timers.

### 11.5 What it does not do

It does not stop a `sleep`-based loop the program wrote itself, a request already in flight, or `start`. It does not read
the Battery or Network Information APIs. A page that is visible but covered (a window behind another, a split screen) is
active: the browser says so, and so does this.

### 11.6 Checks planned

Unit: the parser is unchanged; replay: hide, an `advance` across several periods, show (one catch-up tick; none when not
due; none while a run is in flight; none after dispose), a second `hide`, an `advance` shorter than the time left. Browser
(Chromium, `page.clock` and a faked `document.visibilityState`): the same cases through the emitted code, and
`pagehide`/`pageshow`.
