# cancho

> Tools built for agents, not adapted from humans: a native systems language, and the ecosystem built with it, where trust comes from verification, not comprehension.

Status: pre-1.0. As of 2026-11-27. Alpha products are built with it; the compiler is written in cancho, with no Rust, release 0.1.1, built from a committed seed with clang alone; it writes code with LLVM for Linux and macOS, and a backend of its own is started and generates no code yet. cancho-os, an operating system written in cancho with no Linux and no C, has booted one real Raspberry Pi 4.

## The idea

- Built for agents, not adapted from humans: you should not have to read what an agent wrote to trust it. The compiler verifies it, says what it may do, and answers in data. Comprehension is one basis for trust; verification is another. The case: https://alpibru.com/manifesto
- A value is consumed exactly once. A `res` value must be consumed on every path; there are no destructors, no garbage collector, no exceptions.
- Reaching the outside world (console, file, network, foreign code) requires holding a capability for it. `split(world)` is the only source of authority, and every piece must be handed on or `release`d.
- Effect rows are exact: performing an effect you did not declare, or declaring one you do not perform, is refused.
- Every refusal has a stable rule tag (for example `linear-value-unconsumed`, `effect-not-declared`). Match on the tag, not the prose.
- Dependencies pin to content hashes. There is no version string.

## What is new

- **The compiler is cancho, and only cancho.** 158 files and about 80,000 lines of it. clang builds a committed seed, the seed builds the compiler, and that compiler builds itself again, byte for byte the same. No Rust is left in the compiler or in its gate; the Rust still in the repository is comparison code (the other side of three benchmarks, of the TLS interop scripts and of one float test). ([where it stands](https://github.com/alpibrusl/cancho/blob/main/docs/status.md), [the seed](https://github.com/alpibrusl/cancho/blob/main/docs/seed.md), [the gate](https://github.com/alpibrusl/cancho/blob/main/docs/gate.md))
- **Bare metal is a target.** `--target aarch64-none-elf` writes an object with no libc in it, and hardware is two capabilities: `Mmio`, bounded to exactly the registers a driver uses, and `Cpu`, the processor’s own registers by name. On `main`; not in release 0.1.1. ([freestanding](https://github.com/alpibrusl/cancho/blob/main/docs/freestanding.md), [Mmio](https://github.com/alpibrusl/cancho/blob/main/docs/mmio.md), [Cpu](https://github.com/alpibrusl/cancho/blob/main/docs/cpu.md))
- **cancho-os: an operating system with no Linux and no C.** A kernel written in cancho booted a real Raspberry Pi 4 from its own SD card, joined a network through its own Ethernet driver and gave its shell over SSH; a new kernel is sent to it over SSH and it restarts into it. That is one board, one card and one day of sessions; the rest is measured in QEMU on every change. What is not cancho is one page of boot assembly and the board’s firmware. ([cancho-os](https://github.com/alpibrusl/cancho-os#readme), [what the kernel can touch](https://github.com/alpibrusl/cancho-os/blob/main/docs/authority.txt))

## What is next

- **A backend of cancho’s own, without LLVM.** Today the compiler writes LLVM text and clang makes the machine code. A backend written in cancho is being built, AArch64 first, so that a kernel can rebuild itself on its own board. Built: the design, a reader of the LLVM text the compiler writes, and the AArch64 encoder, held to clang’s assembler over 6,144 instructions. Not built: the code generator, objects and images (steps N3 to N15), and nothing calls the encoder yet. LLVM stays the default until a measured gap says otherwise. ([the design and its status](https://github.com/alpibrusl/cancho/blob/main/docs/native-backend.md), [cancho#840](https://github.com/alpibrusl/cancho/issues/840))
- **cancho-os: the safety core as the only holder of the servo bus.** The kernel reads two real SO-101 arms and judges a pose with cancho-robot’s safety check, and it cannot move a servo. Next is sending a motion, through the safety core or not at all. ([cancho-os roadmap](https://github.com/alpibrusl/cancho-os/blob/main/docs/ROADMAP.md))
- **cancho-os: separate programs, the Pi 5, a second target.** Programs run inside the kernel today, on a system lent to them for a block. Not done: separate programs, signed grants and approval, USB, and the Pi 5, whose code is written from the board’s device tree and has never run. A PC with UEFI is the planned second target. ([cancho-os roadmap](https://github.com/alpibrusl/cancho-os/blob/main/docs/ROADMAP.md))
- **The language is pre-1.0, with no date for 1.0.** It lands one measured slice at a time; the roadmap is the index of what landed and what is open. ([cancho roadmap](https://alpibrusl.github.io/cancho/ROADMAP.html))

## Commands

```sh
cancho check <file.cho>... [--std] [--output json]   # refuse, or say nothing
cancho build <file.cho>... [-o <output>] [--std]     # native binary
cancho run   <file.cho>... [--std]                   # build, run, exit with the program's status
cancho test  <file.cho>... [--std]                   # run every `fn test_*`; exit 4 if one failed
cancho authority <file.cho> [--std] [--output json]  # what the program may do, and what it never touches
cancho introspect                                    # the command tree, as data
cancho skill                                         # a SKILL.md, as data
```

Start here to write cancho: https://github.com/alpibrusl/cancho/blob/main/AGENTS.md
Docs: https://alpibrusl.github.io/cancho/

## Where to start

### Build software with agents

Start with the language and the agent workflow: describe a task, let an agent build it, read the checks.

- [cancho](https://github.com/alpibrusl/cancho) `built`: Refuses a leaked resource by a named rule, says what a program may do, and builds a native binary. Page: https://alpibrusl.github.io/cancho/ Next: Run the compiler: https://cancho.dev/#try
- [cancho-code](https://github.com/alpibrusl/cancho-code) `built`: In its own evaluation (Claude Haiku 5.5, hidden tests), 13 small libraries from hand-written plans finished 38 of 39 units; conditions and caveats are in its eval/results. Page: https://alpibrusl.github.io/cancho-code/ Next: Paste its agent prompt into your coding agent: https://github.com/alpibrusl/cancho-code/blob/main/docs/agent-prompt.md
- [cancho-tools](https://github.com/alpibrusl/cancho-tools) `early`: A stale write is refused with a rule tag and the hash to retry: two agents, one file (a transcript in its README). Page: https://alpibrusl.github.io/cancho-tools/ Next: Read the two-agents demo: https://github.com/alpibrusl/cancho-tools#one-complete-flow-two-agents-one-file

### Build software infrastructure

Services and tools written in cancho, each with an authority report the compiler derives.

- [cancho-hooks](https://github.com/alpibrusl/cancho-hooks) `alpha`: An event is on disk before the 202 answer, then signed (Standard Webhooks), retried and replayable. No 24-hour soak has completed yet. Page: https://alpibrusl.github.io/cancho-hooks/ Next: Read its landing page: https://alpibrusl.github.io/cancho-hooks/
- [cancho-cache](https://github.com/alpibrusl/cancho-cache) `alpha`: Answers 69 of Redis 7.0’s 240 commands over RESP2 and RESP3; its authority report, checked in CI, names the one file it reads. Page: https://alpibrusl.github.io/cancho-cache/ Next: Try it from redis-cli: https://github.com/alpibrusl/cancho-cache#readme
- [cancho-table](https://github.com/alpibrusl/cancho-table) `early`: A wrong column name is refused with a rule tag and the corrected command as a repair. Page: https://alpibrusl.github.io/cancho-table/ Next: Read its landing page: https://alpibrusl.github.io/cancho-table/

### Explore new computing boundaries

Hardware, GPUs, robotics and an operating system. Experimental: read each status before you rely on it.

- [cancho-os](https://github.com/alpibrusl/cancho-os) `built`: Booted a real Raspberry Pi 4 and gave its shell over SSH, with no Linux, no libc and no C; every change to a file keeps the old version and a line in a record of who made it. Page: https://alpibrusl.github.io/cancho-os/ Next: Read where it stands: https://github.com/alpibrusl/cancho-os#readme
- [cancho-hdl](https://github.com/alpibrusl/cancho-hdl) `built`: A .chw contract is checked (a field overlap is refused as `field-overlap`), and a C header, device tree, SystemVerilog and CMSIS-SVD are generated from it. Page: https://alpibrusl.github.io/cancho-hdl/ Next: See its refusal examples: https://github.com/alpibrusl/cancho-hdl#readme
- [cancho-gpu](https://github.com/alpibrusl/cancho-gpu) `built`: CUDA C and Metal for 208 test cases (182 emit, 26 refused by both) are byte-identical with an independent implementation’s. Page: https://alpibrusl.github.io/cancho-gpu/ Next: Read its landing page: https://alpibrusl.github.io/cancho-gpu/
- [cancho-robot](https://github.com/alpibrusl/cancho-robot) `design`: Nothing is built yet: research, design stage. Page: https://alpibrusl.github.io/cancho-robot/ Next: Read the plan: https://alpibrusl.github.io/cancho-robot/

## What is verified, and what is not

A passing check shows that the checks listed here passed. It does not show a program is correct or secure in every respect, and tests exercise the behaviour they cover, not all behaviour. None of this is a certification, and no independent audit is claimed.

### Verified by the compiler

- A value is consumed exactly once. A resource left unconsumed is refused as `linear-value-unconsumed` (19 refusal fixtures). ([rule fixtures](https://github.com/alpibrusl/cancho/blob/main/docs/agent-errors.md))
- Effect rows are exact. Performing an effect the signature does not declare is refused as `effect-not-declared` (12 fixtures); declaring one that is not performed is refused too. ([rule fixtures](https://github.com/alpibrusl/cancho/blob/main/docs/agent-errors.md), [authority](https://github.com/alpibrusl/cancho/blob/main/docs/authority.md))
- Authority is derived, not declared: `cancho authority` lists what a program can reach, and says “unbounded” when foreign code is reachable. ([authority](https://github.com/alpibrusl/cancho/blob/main/docs/authority.md), [foreign code](https://github.com/alpibrusl/cancho/blob/main/docs/foreign-authority.md))
- Every refusal carries a stable rule tag, and `--output json` gives it as data. ([the contract](https://github.com/alpibrusl/cancho/blob/main/docs/agent-errors.md))
- Each declaration has a content hash (`cancho ids`); dependencies pin to hashes. ([canonical AST (status: proposed)](https://github.com/alpibrusl/cancho/blob/main/docs/canonical-ast.md))

### Exercised by tests

- 446 refusal fixtures, each naming the rule it must be refused with, and 134 accepting programs. ([tests/reject](https://github.com/alpibrusl/cancho/tree/main/tests/reject), [tests/accept](https://github.com/alpibrusl/cancho/tree/main/tests/accept))
- The fixtures are real programs, built and run; the gate (format, file budget, links, the seed, the fixtures, the compiler’s tests) is written in cancho and shell, with no Rust, and runs on linux-x86_64 and darwin-aarch64. ([the gate](https://github.com/alpibrusl/cancho/blob/main/docs/gate.md), [CONTRIBUTING](https://github.com/alpibrusl/cancho/blob/main/CONTRIBUTING.md))
- Every checked code block in AGENTS.md is a fixture; changing a rule name to a wrong one turns the suite red (checked by doing it). ([AGENTS.md](https://github.com/alpibrusl/cancho/blob/main/AGENTS.md))
- A mutation fuzzer feeds token-level edits of every program to the compiler, requiring a stable print-and-parse, a rule tag on every refusal (none of them `internal`), and that what the checker accepts the backend compiles. ([fuzzer](https://github.com/alpibrusl/cancho/blob/main/tests/commands/fuzz.sh), [design](https://github.com/alpibrusl/cancho/blob/main/docs/fuzzing.md))
- Tests are checked by breaking what they cover and watching them fail. ([CONTRIBUTING](https://github.com/alpibrusl/cancho/blob/main/CONTRIBUTING.md), [differential testing](https://github.com/alpibrusl/cancho/blob/main/docs/differential.md))
- The gate builds the compiler from the committed seed, requires it to reproduce itself byte for byte, and passes 133 of the 134 programs that must be accepted and 437 of the 446 that must be refused by the rule they name; the ten it leaves out are listed with their reasons (measured 2026-11-27, Linux x86-64). ([the gate](https://alpibrusl.github.io/cancho/gate.html), [what is left out](https://github.com/alpibrusl/cancho/blob/main/tests/run.known))

### Still being established

- The compiler writes code with LLVM only, for Linux (x86-64, aarch64) and macOS (Apple silicon), and has no WebAssembly output. A backend of its own is started (a reader and an AArch64 encoder) and generates no code yet. ([README](https://github.com/alpibrusl/cancho#install), [native backend](https://github.com/alpibrusl/cancho/blob/main/docs/native-backend.md))
- cancho-os has run on one real Raspberry Pi 4, for one day of sessions; everything else about it is measured in QEMU. Its Pi 5 code has never run, it cannot move a servo, and an agent’s key has been tried only in QEMU. ([cancho-os](https://github.com/alpibrusl/cancho-os#readme))
- Hash stability is not a contract yet: the canonical-AST document is “proposed”, and 39% of this repository’s own historical revisions do not type-check under today’s build. ([canonical AST](https://github.com/alpibrusl/cancho/blob/main/docs/canonical-ast.md), [hash stability](https://github.com/alpibrusl/cancho/blob/main/docs/hash-stability.md))
- A cancho program has one network bound shared by listening and connecting, so a proxy’s authority report cannot say which upstreams it reaches; cancho-gateway enforces its compiled-in list by exact match and tests it, but the compiler does not vouch for it. ([cancho-gateway](https://github.com/alpibrusl/cancho-gateway))
- The TLS stack is not independently reviewed (stated in cancho-fetch’s README). ([cancho-fetch](https://github.com/alpibrusl/cancho-fetch))
- Not evaluated: whether agents do better with cancho-tools than with the usual tools. That needs an agent-in-the-loop evaluation that has not been run. ([cancho#228](https://github.com/alpibrusl/cancho/issues/228))
- Pre-1.0, and its own README says it is not a usable language yet. ([README](https://github.com/alpibrusl/cancho#readme))

Counted at cancho 39afb52, 2026-11-27. Each line links its evidence; a claim found false is corrected here, in place.

## Ecosystem

Stages: `working` = Does what its README claims.; `alpha` = Runs, may change, not for production.; `early` = Built and tested, with named gaps.; `built` = Code exists and is documented; its README does not name a stage.; `design` = A design or plan; little or nothing is built.

### Language and compiler

- [cancho](https://github.com/alpibrusl/cancho) `built`: The language and its compiler. The compiler is written in cancho and built from a committed seed with clang alone; no Rust is left in it or in its gate. Release 0.1.1. Pre-1.0. Page: https://alpibrusl.github.io/cancho/
- [cancho-ui](https://github.com/alpibrusl/cancho-ui) `built`: A UI language whose compiler knows what each component is allowed to do. Builds JavaScript and DOM. Authority: a component declares `requires Clock;`; a missing or unused declaration is refused (capability-missing, capability-unused). Page: https://alpibrusl.github.io/cancho-ui/

### Services

- [cancho-hooks](https://github.com/alpibrusl/cancho-hooks) `alpha`: Webhook delivery: on disk before the 202, signed, retried, dead-lettered and replayable. Not for production yet. Page: https://alpibrusl.github.io/cancho-hooks/
- [cancho-cache](https://github.com/alpibrusl/cancho-cache) `alpha`: Redis-compatible cache over RESP2 and RESP3: strings and hashes, expiry and LRU eviction, in a fixed arena. Authority: reads one file (/dev/urandom, for the hash seed); no FFI; checked in CI. Answers 69 of Redis 7.0's 240 commands; no persistence, replication or TLS; not a Redis replacement. Page: https://alpibrusl.github.io/cancho-cache/
- [cancho-gateway](https://github.com/alpibrusl/cancho-gateway) `early`: An HTTP/1.1 reverse proxy you can audit: strict, bounded request parsing, no FFI, a checkable authority report. A working slice, not a finished one. The report proves it touches no files and no foreign code; the upstream set is compiled in and enforced by exact match, but the compiler does not vouch for it. Page: https://alpibrusl.github.io/cancho-gateway/
- [cancho-mqtt](https://github.com/alpibrusl/cancho-mqtt) `alpha`: An MQTT broker you can reason about: 3.1.1 over plain TCP, QoS 0 to 2, retained messages, wills and persistent sessions, on one thread with bounded memory. Authority: cannot touch a file, dial out or call foreign code (proved by the compiler). Alpha, for a trusted network: plain TCP, in memory, no TLS, no long soak run. Page: https://alpibrusl.github.io/cancho-mqtt/
- [cancho-dns](https://github.com/alpibrusl/cancho-dns) `early`: A forwarding and caching DNS resolver: a bounded packet parser, an answer cache in a fixed arena, no FFI. Its README still says design stage. Built so far: the codec, a UDP and TCP server, an answer cache, a forwarder and an access list with response-rate limiting. No benchmark yet. Page: https://alpibrusl.github.io/cancho-dns/
- [cancho-oci](https://github.com/alpibrusl/cancho-oci) `alpha`: Container images without a daemon: oci-build assembles a reproducible FROM-scratch image from static executables. Authority: no network in its authority report. Alpha; oci-build works.
- [cancho-forge](https://github.com/alpibrusl/cancho-forge) `built`: An agent-run software company you can audit: state, budgets, approvals and evidence are kept by the runtime, and a worker cannot accept its own work. Authority: the dispatcher holds an Exec narrowed to the worker's own programs; no filesystem, no network (Milestone 0 spike). Milestone 1's core only: a task state machine, an append-only journal with crash recovery, snapshots, and a read-only status and history. Page: https://alpibrusl.github.io/cancho-forge/

### Libraries

- [cancho-pg](https://github.com/alpibrusl/cancho-pg) `early`: PostgreSQL client: v3 protocol, SCRAM-SHA-256, extended queries, TLS, non-blocking pool. Authority: network and heap only; no C or foreign calls.
- [cancho-schema](https://github.com/alpibrusl/cancho-schema) `early`: Schemas as data: validation, problem+json errors and JSON Schema 2020-12 from one value. Authority: heap only; no C.
- [cancho-web](https://github.com/alpibrusl/cancho-web) `alpha`: Typed routes, validated bodies, problem+json errors and an OpenAPI document from one declaration. Authority: one thread, no FFI. Rests on: cancho-schema. The declaration half of the framework is built. Page: https://alpibrusl.github.io/cancho-web/
- [cancho-log](https://github.com/alpibrusl/cancho-log) `early`: A durable, segmented, append-only log with crash-safe recovery that cuts off a torn tail. Authority: no FFI. Append path only; not a Kafka replacement.
- [cancho-stats](https://github.com/alpibrusl/cancho-stats) `built`: Statistics whose last digit you can check: a command line for describing data and drawing inferences, with exact sums and error bounds written before the code. Its page replays two real runs of stats describe. Page: https://alpibrusl.github.io/cancho-stats/
- [cancho-ml](https://github.com/alpibrusl/cancho-ml) `alpha`: Classical machine learning on tabular data as a command line an agent can read: seven models, one JSON line out, or a named refusal with a repair. Early, alpha, not for production. Page: https://alpibrusl.github.io/cancho-ml/
- [cancho-mqtt-client](https://github.com/alpibrusl/cancho-mqtt-client) `design`: An MQTT 3.1.1 client library: connect, publish, subscribe, will, QoS 0 and 1, and a reconnect with bounded backoff. The other half of cancho-mqtt.

### Tools

- [cancho-tools](https://github.com/alpibrusl/cancho-tools) `early`: Ten small tools for agents (seek, list, peek, jsonq, tally, hash, write, replace, move, archive) and one opt-in, run. JSON out, tagged refusals that often carry a repair. Authority: a compiler-proven authority per binary. Not claimed: that agents do better with them than with the usual tools. That evaluation has not been run. Page: https://alpibrusl.github.io/cancho-tools/
- [cancho-table](https://github.com/alpibrusl/cancho-table) `early`: Select, filter, group and aggregate CSV/TSV for agents; bounded, exact, byte-stable. Rests on: cancho-tools. Page: https://alpibrusl.github.io/cancho-table/
- [cancho-code](https://github.com/alpibrusl/cancho-code) `built`: An AI coding assistant that proves its work: the cancho compiler checks every piece the model writes, and a task is done only when the compiler and the task's own examples say so. Its README reports 38 of 39 units verified on 13 small libraries built from hand-written plans (Claude Haiku 5.5, hidden tests); conditions and caveats are in its eval/results. Page: https://alpibrusl.github.io/cancho-code/
- [cancho-client-gen](https://github.com/alpibrusl/cancho-client-gen) `built`: Reads the OpenAPI 3.1 document that cancho-web generates and writes strictly typed clients; the contract, both ways. Authority: the clients it writes make no foreign call. Rests on: cancho-web.
- [cancho-fetch](https://github.com/alpibrusl/cancho-fetch) `design`: An agent-safe fetch: HTTP and HTTPS to a fixed, built-in list of hosts, with the list in the binary's authority row instead of a flag. Rests on: cancho-tools. Design only; the host matching it needs is in review in cancho (#561).

### Hardware and kernels

- [cancho-os](https://github.com/alpibrusl/cancho-os) `built`: An operating system written in cancho, on bare metal: no Linux or libc under it and no C in it. The drivers, the page tables, the network stack, SSH and the shell are cancho; what is not is one page of boot assembly and the board’s firmware. Authority: the kernel has no extern fn and no Ffi, so the report says it never touches foreign code, and it lists the hardware registers the kernel can reach; checked in CI. Rests on: cancho-tools, cancho-robot. Seen on one real Raspberry Pi 4 (one board, one card, one day of sessions); the rest is measured in QEMU. It reads two SO-101 arms and cannot move them. Not yet: separate programs, USB, the Pi 5 (its code has never run). Page: https://alpibrusl.github.io/cancho-os/
- [cancho-robot](https://github.com/alpibrusl/cancho-robot) `design`: A robot arm controller: the Feetech servo bus, SO-101 kinematics and a safety core every motion goes through. Research, design stage. Page: https://alpibrusl.github.io/cancho-robot/
- [cancho-gpu](https://github.com/alpibrusl/cancho-gpu) `built`: A compiler for typed tile kernels (.chx) to CUDA C and Metal, written in cancho; it also runs language models on a Metal GPU. Per its README: Qwen2.5-0.5B and a 27B model agree with reference implementations. Page: https://alpibrusl.github.io/cancho-gpu/
- [cancho-hdl](https://github.com/alpibrusl/cancho-hdl) `built`: One .chw file declares the software/hardware boundary (registers, fields, interrupts, DMA); it is checked, and a C header, device tree, SystemVerilog RTL and CMSIS-SVD are generated from it. Being ported from Python to cancho: the parser, validator and checker are ported; the generators, diff and report are still Python. Page: https://alpibrusl.github.io/cancho-hdl/

More, including repositories not listed here: https://github.com/orgs/alpibrusl/repositories?q=cancho-&type=public

## Measured

On Linux x86-64, the compiler built from the committed seed; CI runs the same gate on linux-x86_64 and macOS aarch64, 2026-11-27 (moves as the work does; https://github.com/alpibrusl/cancho/blob/main/docs/gate.md):

- 133 / 134 programs that must be accepted are built and run with the output their directives state, by the gate on every push; the one left out is a program the compiler cannot build yet.
- 437 / 446 programs that must be refused are refused, each by the rule its file names, by the gate on every push; the 9 left out name a WebAssembly target, which the compiler has not.
- 59 named rules the refusal programs exercise, stable tags a program can match on.

Record: https://alpibrusl.github.io/cancho/gate.html

## Machine-readable

- https://cancho.dev/data/ecosystem.json: every project above as JSON
- https://cancho.dev/data/demos.json: recorded command sessions (real output) for the projects that have one
- https://cancho.dev/llms.txt and https://cancho.dev/llms-full.txt
