# Focus, announcements and the page title when a screen changes

> **Status: BUILT** (issue #92; design first, built in the same PR, section 8 lists what the build corrected; found building `examples/tickets`, `docs/tickets.md` 3.4; routing, `docs/routing.md`, made it
> matter more). Written 2026-10-23 before the code ([`CONTRIBUTING.md`](../CONTRIBUTING.md) section 1). Numbers were measured in Chromium (Playwright,
> `Pixel 7` profile) on main at 90945d6 with two small programs: a sign-in form that swaps for a work-order screen (`If`), and `examples/routes`.
> Guarantee tier of the page behaviour: **observed in Chromium**. Replay cannot claim it (section 5). No screen reader was run (section 7).

## 1. What happens today, measured

| situation | `document.activeElement` | `document.title` | live regions |
|---|---|---|---|
| sign-in: Enter in the user field replaces the form with the next screen | `INPUT` before, **`BODY`** after | the program's name, unchanged | 0 |
| sign out from the keyboard (the button disappears with its screen) | **`BODY`** | unchanged | 0 |
| routes: tap a link (the link is gone with its screen) | **`BODY`** | unchanged (`routes`) | 1 (a status `Text`, not about the screen) |
| routes: browser Back, a typed deep link | `BODY` (it was) | unchanged | 1 |

* A heading is `<span role="heading">` with no `tabindex`; Chromium's accessibility tree gives it `level=2` (the ARIA default) without an `aria-level` attribute.
* A heading can take focus from a script once it has `tabindex="-1"` (`activeElement` is then the heading). The focus ring that results depends on how the user got there,
  not on the element: after a **touch** tap `:focus-visible` is `false` and no outline is drawn; after the **keyboard** (Enter) it is `true` and Chromium draws its default
  `auto 1px` outline.
* Nothing in the language says where focus should go, and nothing announces a change of screen.

So a keyboard user is returned to the top of the page and a screen-reader user hears nothing about a change that sighted users see. The title never describes the screen,
which matters for a routed app: its URLs are different pages.

## 2. The options

| | Option | For | Against |
|---|---|---|---|
| A | **Move focus to the screen's heading, when focus would otherwise be lost, and on a route change** (chosen) | the standard remedy for a single-page app; the move is itself the announcement (a screen reader reads what takes focus); no new syntax; nothing changes for a page where focus is not lost | needs a heading to point at; "the first heading" is a convention (section 4) |
| B | A binding that says where (`focus: heading`) | explicit | every program must say it, and a program that forgets is the bug today; one more binding to check |
| C | A polite live region with the new heading's text | no focus movement | measured need is *keyboard* focus (BODY); a live region does not move it; announcing and focusing both would read the heading twice; a region that the page rebuilds at every render is not reliably announced (#106) |
| D | Focus the first control of the new screen | the user can act at once | skips the heading, so a screen-reader user never hears the context; a screen with no control gets nothing; a first control in a form opens the on-screen keyboard on a phone |
| E | Do nothing | | the measurements above |

**Chosen: A.** C is rejected for the reason in its row, not measured with a screen reader (section 7). D can be added later for the one case it fits (a screen that is a single
form) without changing A.

## 3. The rule

1. **Focus lost.** At the end of a render, if the focus was inside the component before it and the rebuild could not give it back to the same control (the control is gone, or
   it is another kind or has another label), focus moves to the heading of the screen that appeared (rule 3). Nothing happens when the focus was elsewhere (the URL bar, another
   component, the body): a timer or a request that redraws the page never takes focus.
2. **Route change.** When a `route` event runs because the fragment changed (a link, Back, Forward, a typed URL, `router.go`), then after its handler has finished, focus moves
   to the first heading, **whether or not it was inside the page**. At mount (the page loaded at a deep link) nothing is focused.
3. **Which heading.** A screen is what an `If` draws. The page records, at each render, the nodes each `If` outside a `For` appended, and which of them were
   not drawn in the render before; the heading is the first `role: heading` in what those just-appeared `If`s drew (kept from the last render in which one
   appeared). A page with no `If` has one screen: its first heading. A screen that appeared without a heading has none (focus is not moved to a heading elsewhere on
   the page, the application's title included).
4. **The heading element** gets `tabindex="-1"` when it is focused (so it takes focus from a script and is not a Tab stop). The page draws no ring of its own: Chromium draws its
   default one after the keyboard and none after touch (section 1).
5. **The title.** A component that declares `route` keeps `document.title` equal to `<heading text> – <the title the page loaded with>` at mount, after each route event, and
   whenever rule 1 moves its focus; a screen with no heading gets the title it loaded with. A component without `route` never touches the title (a title that follows a heading
   changes every page that has one, and only a routed page has screens at different URLs).
6. **No heading, no move.** A screen without a heading keeps today's behaviour (focus is on the body). The fix for that is in the program: a screen has a heading.

## 4. Limits, stated

* A screen is an `If`. A page that swaps its screens some other way (a `For` over a list of one, a component instance, a state that changes only the text) has no
  screen to point at, and gets its first heading; an `If` inside a `For` is not tracked (its key would be the same for every row).
* A button whose label changes when it is pressed (`label:` computed) is "another label" for the restore that already existed, so rule 1 now moves focus to the heading
  instead of dropping it to the body. Neither is right (the control should keep focus); the restore compares labels so that it never focuses a *different* control. Not changed here.
* A route handler that waits for a request (`await`) moves focus when it has finished, not when the first screen appears. That is the point at which the screen is complete,
  but a handler that never finishes never moves it.
* There is no opt-out. A program that wants no move has no heading, or its focus is not inside the page when its state changes.

## 5. Replay

Replay does not draw, so it cannot say which node would hold focus or what the title is, and it does not claim to: the report's `not_claimed` list already says it has no DOM
(`docs/replay.md`). The behaviour is checked in the browser only (`scripts/check-screen-focus.js`). A trace that routes is unchanged.

## 6. What the build must have

`scripts/check-screen-focus.js`, in the mobile profile, each with a deliberate break:

* sign-in and sign-out from the keyboard: the heading of the new screen holds focus, has `tabindex="-1"`, and `:focus-visible` is true; after a touch tap it holds focus and
  `:focus-visible` is false;
* a page whose focus is not inside it (a timer redraws it) is not moved;
* a control that keeps its place keeps focus (typing in a field does not move it to a heading);
* routes: a link, Back, a typed URL each move focus to the new screen's heading and set the title; the deep link at load sets the title and moves nothing; a screen with no heading
  restores the loaded title; a non-routed page's title is never written;
* a screen with no heading leaves focus on the body.

Breaks (`scripts/breaks/screen-focus.json`, eleven): no move on lost focus; a move when focus was elsewhere; a move at mount; no move on a route change; the title not set; the title
set on a non-routed page; the heading not focusable (`tabindex` missing); the title without the loaded title; a focused heading losing its `tabindex` at the next redraw; the first heading
of the page used instead of the screen's. Each is caught.

## 7. Not done, and not verified

* **A screen reader.** Nothing here was heard. That a move of focus to a heading is announced is the established behaviour of the platform; this PR measures the focus, the
  attributes and the accessibility tree, not the speech.
* **iOS Safari and a real phone**; focus rings and `:focus-visible` there are not measured.
* **Skipping to a landmark or the main content**, a "skip link", and scroll restoration on Back.
* **A binding to choose the target** (option B) and a screen that is one form (option D).
* **The level of the heading** (`aria-level`); the tree says 2 for all.

## 8. Corrections

Found building this, in the order found:

* **The first-heading rule was wrong for the flagship example.** Section 3 first said "the first `role: heading` in the component's root". `examples/tickets` draws its
  application title (`Work orders`) as a heading before both screens, so after sign-in and after sign-out the focus went to the application title; `scripts/check-tickets.js`
  measured it. The rule is now "the heading of the screen that appeared" (section 3, rule 3), which needed the page to record what each `If` drew. The orders screen had no
  heading of its own; `examples/tickets` now has `Orders`. The check page `check-screen-focus.js` has an application title above its screens, and a deliberate break puts the
  first-heading rule back.
* **A heading that held the focus lost it at the next redraw.** The restore that keeps a control focused compared the tag and the label and then called `focus()`, but the
  rebuilt heading has no `tabindex`, so `focus()` did nothing and the focus fell to the body after the first request that finished (measured with a 5 ms poll of
  `document.activeElement` through sign-in: input, heading, body). The restore writes `tabindex="-1"` back first. A ticker page that redraws while its heading is focused checks it.
* **Timing, in the check.** `check-tickets.js` read the focus when the text "You are signed out." appeared, which is the render before the screen swap; it waits for the
  heading now.
