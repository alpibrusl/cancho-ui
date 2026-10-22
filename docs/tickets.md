# Work orders: a realistic page built from what exists

`examples/tickets/tickets.cui` is a sign-in form with validation, an
orders list, a detail view, and an edit form that posts with
`net.post`. It is built only from features that exist today. Where the
language fell short, the page does the awkward thing and the gap is a
filed issue (section 3), not a fix in this change.

## 1. What it is

Component `Tickets`, `requires Network; requires NetworkWrite;`. The
service is a stand-in (`scripts/tickets-server.js`): the session is an
HttpOnly, SameSite=Strict cookie, so the program never holds a token.
`net.get` and `net.post` answer an envelope `{"status": .., "body": ..}`
and `json.get` always answers a String.

Three screens, held as lists of zero or one element (`show_signin`,
`show_app`, `show_detail`) and drawn with `For`, because a view has no
conditional (#90).

## 2. The flows

- **Sign in.** `User` and `Pass` validate on every keystroke (message and
  tone per field); `Sign in` is disabled until both are valid. `SignIn`
  posts the credentials, shows the service's refusal on a 401, and on
  success loads the orders.
- **List and detail.** `Pick` selects an order and shows its detail.
- **Edit.** `Status` and `NoteEdit` validate; `Save` posts the change and
  shows the outcome. `SignOut` returns to the first screen.

Trace: `examples/tickets/tickets.trace.json`, 26 steps, 78 checks, run by
`cancho-ui test`. It includes step 6, where the orders request is still
issued after a 401 sign-in (#93).

## 3. Gaps found

Each has a measured reproduction in its issue.

| Gap | Issue |
|---|---|
| A view has no conditional; screens are zero-or-one-element lists | #90 |
| TextField `autocomplete`, `maxlength`, `inputmode`, `name` are not bindings | #91 |
| Focus falls to BODY after sign-in and sign-out; nothing announces the screen change | #92 |
| Handlers cannot share a sequence or make a request conditionally (the orders request is written three times and runs after a failed sign-in) | #93 |
| A status Text with a tone draws its mark when empty | #94 |
| `json.get(reply, "status", "") == 200` (String against number) is accepted and always false | #95 |
| The password field is `type=text` in the built page (`type: "password"` is `binding-unknown`) | #76, #83 |

#83's "sign-in" tile therefore stays a "coming soon" tile: its gap is not
closed. There is no tickets tile in the gallery: the static
`docs/demo/` host has no login route, so a demo there would only show a
refused sign-in.

## 4. How it is checked

- `cancho-ui check examples/tickets/tickets.cui` and
  `cancho-ui test examples/tickets/tickets.cui examples/tickets/tickets.trace.json`.
- `node scripts/check-tickets.js` builds the page and drives it in
  Chromium at 420 px against the stand-in service. Checks that record a
  gap print a `note` instead of failing, so a fix to a gap shows as a
  changed note.
