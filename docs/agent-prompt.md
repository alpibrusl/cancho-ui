Install cancho-ui and try it on one small program. Work in a new empty directory, keep every path free of spaces, and never use sudo. Run steps 2 to 5 in one shell session. Report honestly; do not retry a failing command in a loop and do not edit the repository to make a command pass.

1. Check that `git`, `cargo` (Rust, via rustup) and a C linker (`cc`) exist. If one is missing, stop and tell me which; do not install system packages on your own.

2. Get the repository:
   git clone https://github.com/alpibrusl/cancho-ui.git
   cd cancho-ui

3. Build it and prove the install works. This needs no network after the clone, no key and no model:
   cargo build --release
   cargo test --workspace          # every line `test result:` must say ok, none failed
   export CUI="$PWD/target/release/cancho-ui"

4. Try the compiler on its own example, a clock component that declares `requires Clock;`:
   "$CUI" check examples/clock/clock.cui ; echo "exit $?"                      # expect exit 0
   "$CUI" inspect effects examples/clock/clock.cui Clock --output json        # the capability and where it is used
   Now make a copy without the declaration and check it:
   sed '/requires Clock;/d' examples/clock/clock.cui > /tmp/no-cap.cui
   "$CUI" check /tmp/no-cap.cui ; echo "exit $?"                                # expect capability-missing, exit 1
   "$CUI" check /tmp/no-cap.cui --output json                                   # the same refusal as data (code, span, remediation)
   Then compare two programs. Use examples/clock/clock.cui and examples/counter/counter.cui:
   "$CUI" diff examples/clock/clock.cui examples/counter/counter.cui ; echo "exit $?"     # expect exit 5 (the programs differ)
   "$CUI" diff examples/clock/clock.cui examples/clock/clock.cui ; echo "exit $?"         # expect "no semantic change", exit 0
   Then replay a trace of events with no browser. The trace resolves a slow earlier search after a newer one; the stale answer must be dropped:
   "$CUI" test examples/search/search.cui examples/search/stale.trace.json ; echo "exit $?"   # expect PASS, exit 0

5. Build the counter to JavaScript and look at it in a browser:
   "$CUI" build examples/counter/counter.cui -o docs/demo/counter.js
   python3 -m http.server -d docs 8000        # only serves the files; then open http://localhost:8000/demo/counter.html
   Click Increment twice and Reset. If you cannot open a browser, say so and stop after the build.

6. Tell me, briefly:
   - the exit code and the first line each command printed in step 4;
   - whether `cargo test --workspace` passed, and how many tests;
   - what the counter did in the browser, or that you could not open one;
   - anything that failed, with the exact error.
Docs: README.md (the commands), docs/semantic-diff.md (what diff compares), docs/replay.md (what test replays, and what it does not), spec/diagnostics/README.md (the refusal shape).
