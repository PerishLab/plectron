# Plectron

Read the canonical [PerishLab delivery governance](https://github.com/PerishLab/.github/blob/main/GOVERNANCE.md)
at work start and again before delivery or Issue closure.

## Purpose

Plectron is an executable architecture specification for a modern application UI
runtime and its engineering suite. It absorbs the proven shapes of the Web model,
backed by the smallest working implementation that challenges their boundaries. It is
not the seed of a production Electron replacement or a bid for general browser
compatibility. Rough toy internals are acceptable; unclear boundaries are not. A real
window and script-driven interaction belong to the model's minimum useful expression.

Electron, Blink and other engines are mentors for semantics and design tradeoffs,
not blanket compatibility obligations. test262 inspires executable conformance.
Existing code has no compatibility privilege, and incidental toy behavior must not
silently become normative. Agent-assisted implementation serves model iteration.

## Source of truth

Types, traits, executable state transitions and conformance cases define behavior.
Change affected contracts, implementations and cases together. This file is the
repository's only maintained unstructured guidance: keep durable intent, architectural
constraints and working conventions here. Do not accumulate iteration logs, benchmark
results, completed-work lists or transient next steps here. Do not add separate
architecture documents, READMEs or prose specifications in source comments.

The sole source-comment exception is a local SAFETY explanation immediately adjacent
to an audited native FFI call or thread-transfer implementation in the Metal, machine,
IPC, observation or native platform adapter, within the existing ectropy boundaries. References and evidence must
retain their source and revision when adopting concrete upstream behavior.

A compiling interface is not proof of behavior. Keep absent, excluded, supported
subset and verified capability distinct. Consult executable capability profiles and
tests instead of preserving an independently maintained prose implementation inventory.

## Architectural boundaries

The runtime role is independent of engine choice: JS Runtime = JS engine + addons.
Node plus platform capabilities is an intended real-world route; QuickJS illustrates
another choice. Neither Node nor Chromium is a dependency requirement for the toy.
An addon publishes named capabilities into one frozen host registry. The JavaScript
package `@plectron/engine` owns their application-facing names and composition; application
source imports capabilities from that package instead of depending on ambient globals.
Runtime construction does not assign a process identity or decide which capabilities may
be composed together. When a composition uses separate control and rendering runtimes,
objects, closures and native handles cannot cross their engine ownership boundary.
Messages carry copied values or explicit resource references, not shared JS heap objects.
The host registry is installed as an ordinary `Symbol.plectron` keyed property on the
`Symbol` intrinsic and consumed by `@plectron/engine`; tests exercise the package facade.
The symbol follows the same engine-owned identity and property semantics as
`Symbol.iterator`; it introduces no symbol registry, global-object or string-key mechanism.
Application scenarios load files
through the project translator at runtime rather than embedding `apps/` source into test
binaries.
JS values held across runtime calls require explicit roots. Addons report their own
roots and edges back into JS; caches that merely preserve object identity do not
create roots. A connected document tree is live, while a detached tree remains live
through reachable JS wrappers or host references. DOM listeners retain callbacks for
as long as their owning node or explicit subscription scope remains live. JS and DOM
may use separate arenas, but reachability must cross their boundary and collect
unreachable cycles. Native resources continue to use explicit release.

The rendering unit caps rendering-side authority. Control and rendering interact only
through exposed protocols. Logical isolation does not prescribe an OS process layout.
Keep implementation state private; Rust ownership helps implement contracts but does
not replace observable semantics that another language could reproduce.

Runtime crates live under `crates/`; development tooling lives under `tools/`.
Dependencies point from application composition toward engines, never in reverse.
Preserve the following ownership boundaries rather than duplicating their authority:

- `jsengine` owns language execution; `jsruntime` composes it with addons.
- `domengine` owns document state, CSSOM and capture of geometry and paint facts.
- `geoengine` owns platform-independent geometry identity, closed topology facts,
  change planning, immutable generations and geometric computation. Its public requests
  own their inputs so execution may move between the caller and engine workers without
  changing semantics. Geometry uses signed platform-independent logical units; device
  scale and pixel snapping remain downstream. Semantic operations acquire identity only
  when they form an independently invalidated, reusable or observable computation. Published
  states carry an opaque `Stamp`; geoengine preserves it for equivalent published content across
  recomputation, page copies and topology slot remaps, while changed content receives a fresh
  identity. Geometry quality (`estimated`, `measured` or `stale`) is part of published content
  identity even when numeric bounds remain equal. Downstream reuse consumes this identity instead
  of producer object addresses. Geoengine derives immutable,
  origin-independent content signatures from state bounds, attachments and child stamps for
  equivalence checks. Flex distribution receives normalized numeric inputs from DOM and is a
  geoengine computation. Block flow receives an owned request of normalized child bounds and
  returns placements, natural size and overflow; DOM retains CSS interpretation and dependent
  child measurement. Geoengine does not depend on DOM,
  graphics or platform state. Geometry facts validate parent-flow agreement and own a stable
  postorder. Signatures inline zero or one child stamp and allocate a local slice for wider
  nodes. Change plans expose affected computations separately from the ancestor closure required
  to publish a connected measurement tree; containment may stop reflow but not publication. A
  measurement invalidation scope is not automatically a safe content-identity closure
  where layout isolation stops propagation. Facts expose graph depth, wave width and
  fanout so scheduling potential is observable before concurrency is introduced. Paged
  geometry stores report logical, owned and shared residency plus publication copy
  traffic; page width is consumer policy and must be selected from workload evidence.
  Geometry attachments carry stable
  identity without carrying their payload; producers retain payloads in side tables and
  consumers resolve them at their composition boundary. DOM measurement generations pin
  resolved payloads until snapshot publication. Geometry caches retain attachment identities;
  a missing bounded-registry payload is a cache miss, while published snapshots retain their
  own payload side table independently of cache residency. Recurrence screening may place cheap
  rejected geometry in a bounded, content-addressed victim window for the current measurement.
  Its first hit promotes the entry to the persistent cache. Geometry above the persistent
  admission limit enters that window only after a second equal request and never promotes. The
  window reports peak entries and bytes, never contributes persistent residency and clears at the
  measurement boundary. A DOM `Layout` owns its measurement
  generation, which indexes measurement nodes weakly for cache reuse. Published snapshots contain
  geoengine facts, DOM materials and attachment bindings without retaining measurement nodes.
  Projection branches retain geometry stamps and compact fact signatures, so keeping a projection
  alive does not retain the measurement tree.
- `textengine` owns font resources, shaping and design-coordinate outlines, independent
  of DOM, graphics APIs and platform font handles.
- `graphics::shared` owns immutable scenes, visual snapshots, preparation and
  submission contracts, including the CPU reference. `graphics::metal` implements the
  native GPU boundary and configures its drawable layer directly; platform adapters
  attach that layer to their native view tree. Do not replace that learning surface
  with an opaque graphics framework. Compute executor selection and physical presentation
  are independent decisions. A scheduler may overlap executor initialization with content
  construction and may dispatch the same immutable prepared work to another executor after
  an explicit deadline. The first valid result for a visual generation wins; late work is
  retired without rebuilding DOM, layout or scene state. Fixed fallback thresholds require
  workload evidence, and executor completion does not imply surface readiness or visibility.
  On macOS the compute executor creates and retains its Metal device, queue and mutable
  residency state on one worker thread. Only completed immutable work crosses to a surface;
  AppKit view access and `CAMetalLayer` creation, attachment and removal stay on the main thread.
  Its bounded mailbox admits multiple submissions before completion callbacks retire work;
  do not serialize admission by waiting for each command buffer inside the executor.
- `platform::shared` owns native-neutral window identity, device input, focus and
  display port protocols. Port errors remain generic so application composition chooses
  its public error model. This layer does not depend on DOM or Web semantics; Plectron
  converts those facts at its composition boundary. `platform::macos` owns the concrete
  AppKit window adapter, text input bridge and native surface attachment. Add
  other platform modules only with their first concrete implementation.
- `webengine` composes one URL-bound content instance and its rendering runtime.
- `ipcengine` owns communication protocols, channel lifecycle and transport adapters.
  Control messages, file/resource delivery and native shared channels retain distinct
  contracts according to their ownership and lifetime requirements.
- `config` distributes opaque values. Consumers decide whether and when to consume,
  interpret or reject them. Storage success is not consumer acknowledgement; do not
  add central validation, rollback or reliability requirements without a concrete need.
- `shared` holds engine-independent contracts and resources; `grammar` owns reusable
  matching work. Grammar DSLs remove lexer/parser repetition without claiming to
  replace semantic parsing or complex handwritten backends.
- `observe` owns diagnostic sessions, correlated intervals, clock observations and native
  profiling adapters. Engines expose boundaries; recording never owns execution or
  resource lifetime. Disabled observation is transparent; dropped and incomplete data
  remain explicit. Cross-thread context propagation is explicit.
- `pipeline` owns static typed node composition and execution observation. Engine-owned
  nodes retain their algorithms, admission strategies and result ownership. Composition
  determines execution and topology together; transparent nodes call existing work
  directly. Repeated subgraphs have their own observation scope, and submission remains
  distinct from completion polling.
- `lib` composes application control, platform resources and content; `cli` owns the
  application event loop and debugger entrypoint. Input, display and diagnostic output are
  distinct platform ports even when one native adapter implements all three. Native
  input publishes device facts; hit testing, pointer transitions, focus and activation
  remain Plectron semantics. Deterministic virtual sources exercise that same path in
  ordinary tests. Routine verification keeps native processes windowless and exercises
  Metal through offscreen devices or unattached layers. Native windows are reserved for
  attachment and presentation changes; physical focus is reserved for AppKit event and
  text-input bridge changes or cross-boundary diagnosis. Native window operations stay
  on their owning thread, and native resource
  reclamation does not depend on a JS finalizer. Add an audio port with its first
  concrete audio model rather than preserving an empty speculative interface.

`pt` translates template-only `.pt` files to JavaScript at the resource boundary. Each
template function receives a document, constructs a detached DOM subtree from intrinsic
tags, and returns its host and refs. It does not mount, invoke components or dispose.
Formatting-only whitespace between tags is ignored; nonblank text runs are preserved
as text nodes. Literal attributes compile to explicit DOM attribute writes.
Named `@event={handler}` declarations compile to an optional `listen(handlers, scope)`
method on the template result; JavaScript explicitly invokes it and owns the scope.
Independent JavaScript modules own component construction, composition, behavior, event
scopes, mounting and disposal. The caller explicitly places or removes the returned
host. An application `main.ts` mounts its root; loading a module has no mount effect.
This component layer has no authority over DOM engine semantics.
The JavaScript boundary component receives a dedicated host and composes reactive
ownership with component instances. It constructs replacement content while detached,
commits one child replacement and disposes the retired instance. An explicit failed
factory receives the error and reset closure; without one, or when that factory fails,
the component publishes a fixed inert terminal node. Terminal construction does not run
application callbacks. Visual recovery and diagnostic reporting remain independent.
`@plectron/reactive` owns scoped Proxy dependencies and effect lifetimes in JavaScript.
`jsengine` supplies ordinary language primitives; `pt` does not infer reactive state
or acquire component lifecycle authority. Reactive error boundaries are explicit
JavaScript owners. A handled failure retires the owned reactive subtree, continues
unrelated scheduled work and remains resettable until explicit disposal. Reset is
available only after failure and reconstructs setup as a new generation; stale callbacks
remain inert, and a retired parent generation cannot be revived through an old reset.
Handled content failures and failures thrown by their handlers produce
distinct diagnostic records that preserve both errors. A handler failure moves to the
nearest parent boundary, while an error without a boundary reaches the runtime job
rejection channel. Boundary setup receives a callback wrapper that captures its owner at
registration, restores ownership without dependency tracking during the synchronous call
and observes a returned Promise rejection. Ownership does not cross `await`; disposed
wrappers are inert, and later failures from their pending Promises reach the runtime
rejection channel. Template syntax does not own fallback or reset behavior.
`Scope` is the reactive lifecycle owner and is constructed through `new Scope()`.
There is no parallel factory surface. Its module-private state does not make lifetime
depend on a finalizer; applications and components still dispose ownership explicitly.

The CLI is the runtime's engineering control plane. Its long-term relationship to the
engine should resemble Cargo's relationship to Rust: one coherent surface for project,
target, profile, dependency, artifact, run, check, test, benchmark, inspection, trace,
configuration and packaging workflows. It orchestrates stable engine and tool
contracts; it does not own their semantics. Let exploratory tools establish a durable
model before promoting their behavior into the CLI.

Prefer stable data representations and explicit state ownership at boundaries:
constraints and fragments, positioned glyphs, immutable scenes, visual state and
prepared graphics resources. DOM identity, layout identity and visual identity are
different. Visual scrolling can proceed independently of content execution. Keep
input, rendering and resource use tied to the appropriate published or acknowledged
snapshot; an old immutable frame must retain the resources needed to interpret it.
Display owns live scroll offsets and applies tag-independent controller policies published by
content. One controller lease may write policy for a scroll space; user movement suspends and
resumes explicit edge following. Reference components use the same public controller as ordinary
elements. Display projects versioned scroll state from scene ranges. Initial subscription replay,
offset changes and range-only changes enter one ordered per-space journal with explicit causes. A
rendering opportunity coalesces state and asynchronously dispatches an immutable fact only for
subscribed nodes before content rendering; unobserved state remains available for diagnostics
without consuming the content mailbox. Stable scene, layout and viewport sources update offsets
incrementally; an identical refresh is not a projection. Projection work and elapsed time remain
observable independently from emitted facts. Frame mapping reports builds, revisions, processed
spaces and elapsed time separately from scroll projection. Presentation acknowledgment remains a
separate fact. A visual-only viewport update records Content's current visual state without
publishing an identical content snapshot; changed deferred demand resumes ordinary content
publication. Do
not expose synchronous CSSOM layout metrics such as scrollTop, scrollHeight or clientHeight. Visual
rebasing keeps unresolved interiors on the ordinary content-demand path. When user movement
suspends edge following, layout replacement preserves the first visible direct child by DOM
identity and local geometry. Missing anchors use the next then previous direct child with each
candidate's own viewport inset; when no semantic candidate remains, the retained offset and
ordinary range clamping apply. Successful rebases enter the scroll journal with an explicit anchor
cause.

Explicit layout isolation requires externally determined width and either definite height or the
selected `contain-intrinsic-height: auto <length>` estimate, plus a paint clip. Intrinsic estimates
participate in outer flow without constraining demanded content measurement. The layout pipeline
remembers measured boundary geometry by DOM identity, content fingerprint and width. A new boundary
is `estimated`; an unchanged remembered shell is `measured`; changed content or constraints retain
the last geometry as `stale` until viewport demand measures it again. These quality transitions are
part of geoengine identity. Cache reuse validates the expected quality at each isolated shell; a
mismatch invalidates the containing cache path without disabling reuse for unrelated siblings. The
layout reuse report records how many estimated or stale geometries became measured, their total
absolute height error and the largest individual error. Scroll anchoring records the resulting
visual correction independently through the scroll journal. Remembered boundary geometry uses the
shared bounded cache policy; eviction falls back to the declared intrinsic estimate and cannot
change the eventual measured result. Its residency and eviction remain explicit in the estimation
report.
Deferred interiors remain represented in layout; their source changes survive cache
eviction. Visual demand crosses into content computation through a request, and a frame
with demanded but unresolved interiors must not be submitted. Geometry queries may
compute layout without publishing paint. Display supplies the live visual state; content
derives relevant isolation boundaries from immutable layout geometry and includes half a
viewport of logical overscan by default. Published relevance reports distinguish total, relevant,
pending, estimated and stale boundaries without exposing synchronous layout metrics to
applications. Focused nodes and their ancestor path remain demanded independently of
visual intersection. Ordinary content keeps its existing semantics.

## Modern capability selection

Select composable modern semantics and discard historical compatibility paths.
The language profile rejects var, sloppy mode, implicit global assignment and with.
Text uses Unicode scalar semantics rather than UTF-16 code-unit indexing; do not
silently introduce surrogate compatibility. Cross-boundary text conversion is explicit
and fallible. Syntax rejection must precede effects; ordinary language exceptions and
host/budget faults remain distinct.

The unit of selection is a proven architectural shape, not a standards checklist item.
Prefer a capability when it has a stable data representation, an explicit owner and
lifecycle, local failure or absence semantics, and a composition boundary that reduces
cross-layer knowledge. It should support incremental computation or explain several
real application cases without depending on compatibility behavior. Constraints to
fragments, positioned glyph runs, immutable scenes, logical isolation to placement and
request/response semantics independent of transport are representative shapes.

Treat a shape as proven when standards, mature engines and adjacent systems converge
on it, or when focused experiments establish the same properties in this model. Keep
ambiguous candidates isolated as examples or experiments and discuss their semantic
cost before admitting them to the core. Familiarity, implementation convenience and
existence in a browser are not sufficient evidence.

Document.write and parser-stream injection are excluded; runtime content changes use
explicit DOM operations. Float and clear are excluded from the application layout
profile. Text wrapping around graphics is a separate potential capability, not proof
that every float use is obsolete. Navigation and ambient BOM APIs remain outside scope.
Class support builds on the general object/construction model; do not exclude ordinary
constructors merely to simplify a class-only surface.

Form semantics are excluded. Input is a focused single-line editor: its committed
buffer and transient IME composition are distinct, and native adapters translate
platform text indices at the boundary.

Representative application scenarios omit independently complex models such as video,
SVG, Canvas and WebGL. A new semantic model warrants discussion; routine implementation
choices within an agreed model remain autonomous. Existing omissions are not automatic
roadmap promises. Upstream fixtures must not be rewritten to conceal unsupported or
deliberately excluded semantics.

The 200,000 maintained-line reference is revisable, not a hard ceiling or delivery
target. Count implementation, DSLs, tests, harnesses and examples; report unchanged
upstream fixtures and dependencies separately with provenance. Evaluate growth by
expressive value and clarity. Never hide implementation in fixture directories or
compress code to manipulate measurement.

## Performance and evidence

Performance mechanisms should have explicit identity, ownership, invalidation and
observation boundaries. Caches are disposable acceleration state; misses and eviction
must preserve semantics. Missing change history requires reconstruction, not an empty
change set. Integer slots alone do not prove reusable content. Resource identity is
distinct from URL, placement and device residency. Cache accounting is not process RSS.

Evolve asynchronous rendering in dependency order. First establish fresh-process,
content, scene, submission and presentation observations. Then define immutable animation
plans, target generations, cancellation and effect publication before isolating sampling
behind a mailbox. Isolation means independent ownership and a bounded protocol; it does
not require a permanent OS thread. Add coalescing, backpressure and visibility policy
before using CPU and memory evidence to tune retained structures and startup. Admit each
additional independently sampled property only after proving that its effect does not
require style, layout or paint reconstruction.

DOM and CSSOM own animation declarations and compile plans. The animation executor owns
time sampling but no document, layout or mutable scene state. Render owns application of
visual effects, while presentation supplies pacing observations. Control transitions
such as replacement, pause and cancellation are ordered and reliable; sampled effect
updates may coalesce to the newest value within the same target generation. Stale target
generations are discarded without reviving retired visual state. Startup optimization
preserves separately observable semantic activation, first scene, first submission and
first successful presentation milestones.

The application event loop combines content pacing as idle, continuous or a monotonic
deadline. A native presentation source supplies due pulses and stays dormant without
demand. Cross-thread producers use a platform-neutral notification capability; the
platform adapter maps it to its native event source. Deadline ownership stays in the
application event loop rather than an animation executor timer.
Native continuous pacing differs by platform: macOS pulses from the display link, Linux
pulses from X11 Present `NotifyMSC` completions when the extension is present and falls
back to the 16 ms timer otherwise, and Windows stays on the 16 ms timer. Xvfb vblank is
simulated (`-fakescreenfps`, 60 Hz by default), so its cadence is not physical refresh
evidence; on hosts with hardware vblank a blanked or off-screen CRTC may slow Present's
fallback vblank to about 1 Hz.
Text composition has one browser-aligned contract on every platform. A session is one
`Composition` id: its first `Mark` is `compositionstart`, every `Mark` is a
`compositionupdate` carrying the whole preedit and its selection, `Commit` is
`compositionend` with the inserted text and `Cancel` is `compositionend` with none. The key
that opens a session is reported as keyboard input before its first `Mark`; keys the open
session consumes produce no keyboard input, so there is no keydown 229 or `isComposing`
stream, and text that needs no session arrives as plain text input. A field switch through
`Input::focus` cancels the open session instead of committing it, and stale events for an
ended id are ignored. macOS maps `NSTextInputClient` marked text onto this contract. Linux
maps dead keys and Compose sequences from the xkbcommon compose state machine onto it: the
preedit is the spacing form of each pending dead key (`·` for Compose and keys without one),
a sequence the table rejects cancels, and window focus loss, a keymap change or a
Control/Alt chord also cancels. `libxkbcommon.so.0` is opened at runtime and fed keysyms
from the core `GetKeyboardMapping` decoding, with no `xkb_keymap` and no
`libxkbcommon-x11`. The deployment dependency is the `libxkbcommon0` runtime library plus
the system X11 locale Compose tables (or `XCOMPOSEFILE`/`~/.XCompose`); no development
package is needed, and keys pass through uncomposed when either is missing. XIM/IBus
preedit on Linux and TSF on Windows are deferred.

For a native frame query record, `admitted` is the submission timestamp immediately
before backend rendering, `produced` is backend completion and `detected` is the point
where Plectron has consumed that completion and published the page state needed for
logical input. These are platform-lifetime monotonic timestamps. Backend completion does
not claim hardware scanout or physical visibility. A controlling process may observe the
record later; that polling delay is a separate upper-bound measurement. The startup
harness reads the retained first-frame record rather than the latest record so animation
frames cannot replace the sample.

Promote tunable policy numbers into named consumer defaults and distribute overrides
through the configuration boundary as needed. Keep semantic constants distinct from
policy. Deliberately invalid configuration is a useful experiment; do not silently
clamp workloads, hide consumer rejection or impose speculative high-level safeguards.

Use orthogonal application scenarios to assess common gains and regressions. Separate
warm repeated work from fresh-process startup. Record workload, configuration, assets,
build mode, executable fingerprint, sample count and measurement endpoint. Layout
benchmarks additionally record graph shape, phase timings, storage residency and copy
traffic. Matched end-to-end observations are the source of truth for policy outcomes;
phase timings and work counters locate causes, while temporal correlation alone does not
assign a regression to the mechanism under test. Layout measurement reports distinguish
semantic budget charge from actual algorithm operation counts. Opt-in bounded causality traces
use geometry identity, content fingerprint and request fingerprint to connect cache admission
with actual recomputation; object addresses are not an observation contract. Their default
diagnosis groups exact request variants, miss causes, within-measurement repetition and interval
recurrence; raw events require an explicit option. A fresh process is not a cold OS
or driver cache. Increase sampling when needed to expose jitter; retain negative and
inconclusive results rather than manufacturing a universal performance claim.

Benchmarks delineate the selected UI-runtime performance domain. Expand that domain by
application relevance and orthogonal workload dimensions, not by browser feature count.
Within an established domain, every compared action must preserve its absolute
advantage over the matched Chromium reference before the next expansion; an average
cannot hide a losing action. Keep semantics, delivered output and measurement endpoints
equivalent, and admit optimizations by capability and dataflow rather than scenario
identity. This establishes a bounded product claim, not universal browser superiority.

Rendering completion, hardware GPU execution, host-observed waiting, readback and
physical presentation are different observations. Do not double-count nested stages,
add phase medians into an invented sample, compare unlike endpoints as a speed ratio,
or move required work outside the interval and claim a gain. A small local improvement
does not establish overall optimality or meet an unrelated end-to-end target.

Content computation counters accumulate successful layout and paint passes over the
content instance's lifetime. Measure an action by subtracting snapshots from that same
instance. Keep these cumulative observations distinct from the last-pass engine reports;
a no-op checkpoint must not erase earlier demand work within the measured action.

Correctness and lifecycle contracts are hard checks. Work counters and timing are
observations unless an explicit contract says otherwise. Report measured CPU/Metal
channel differences and accept the established renderer-specific tolerance; do not
require bit equality or silently loosen checks to conceal a new discrepancy.

Reuse the scenario and experiment tools for launch loops, distribution summaries,
output/work comparisons and failure reports. Keep specialized external oracles where
their tools match the boundary. Preserve raw evidence in distinct output directories;
do not overwrite failed runs or reinterpret historical tests as fresh verification.

## Working cadence

Prefer substantial capability batches that challenge an architectural boundary across
JS, CSS, typography, rendering, resources or isolation. Refine local details during
implementation. Avoid low-return tuning of the most recent mechanism and forced
abstractions that blur ownership. Methods, structs, traits and macros are useful when
they remove genuine repetition while preserving clear boundaries.

Use focused checks during development; concentrate full workspace and expensive GPU
verification at coherent delivery boundaries. Do not repeat GUI automation after the
intended interaction has been confirmed without a new visual concern. Inspect actual
screenshots when making visual claims. Diagnose and close owned processes through the
existing control interfaces rather than leaving background experiments suspended.

Raise genuine semantic or ownership decisions with concrete examples, consequences
and a recommendation while continuing independent work. Ordinary reversible edits,
reviews, fixes and local commits remain within autonomous implementation scope.

## Code style and commits

Plumb governs repository shape, staged proof and delivery. `plumb.toml` declares its
seats and proof lanes; use the Plumb-projected hooks and keep the declaration aligned
with real ownership. Plectron consumes the Plumb Rust substrate for command surfaces
without transferring engine semantics into governance. Keep dependencies minimal and
commit Cargo.lock. The toy is not a security sandbox; implementation budgets do not
imply production-grade isolation or reliability.

Ectropy is a standalone development tool, not a Cargo dependency. Its scan scope,
limits, syntax grants and naming policy live in `ectropy.toml`. Fix findings rather
than adding broad exemptions. Use namespace context for concise single-word names.

Completed changes are committed locally by default after required checks pass. Keep
commits coherent and report their hashes. Publishing and pushing are separate actions.
Project the staged-snapshot hooks in each checkout:

```sh
plumb configuration install .
```

Plumb Guard checks an isolated snapshot of the Git index. `tools/check` remains the
product-owned affected planner and turns staged
changes into an explainable affected plan: formatting, ectropy, changed crate Clippy,
direct consumer checks and capability tests. Unknown executable changes and workspace
infrastructure escalate to the full suite. `cargo verify plan --staged` previews the
plan; `cargo verify run --full --staged` explicitly runs the full suite. The hook never
formats or stages automatically. Keep required tools on PATH and stage all relevant
inputs before invoking it. Do not edit or stage files while Guard runs. Unstaged files
must not mask a staged defect. Governance proof is not a sandbox for untrusted build
scripts.

Plumb owns build settings during Guard. Do not inject Cargo profile or incremental
environment variables into commits. Enable native diagnostics on the explicit native
verification command that needs them, then commit through the projected hook:

```sh
MTL_DEBUG_LAYER=1 MTL_SHADER_VALIDATION=1 cargo verify run --full --staged
git commit
```

## Verification entrypoints

Run checks appropriate to the change; the affected commit hook is the ordinary
delivery check. Run the explicit full suite at coherent delivery boundaries.
Avoid repeating costly runs without a new change, failure or unresolved concern.

```sh
cargo fmt --all -- --check
ectropy .
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo conformance
cargo spec matrix
cargo spec budget
cargo spec scenarios
```

`cargo model dev start apps/counter` runs a configured native interaction app. `cargo model ctl`
provides local process inspection and control. `cargo spec scenario`, `summarize`,
`compare`, `layout-compare`, `layout-cause` and `experiment` provide reproducible scenario checks and analysis;
checked-in plans live under `tools/spec/resources/experiments/`.
Repository Cargo aliases resolve locked local workspace builds; `cargo serve` serves
example resources over loopback HTTP.
Use `cargo spec trace PLAN --output DIRECTORY --options JSON` for bounded portable
interval reports. `cargo observe record PLAN --output DIRECTORY`
records macOS CPU Counters and correlates native samples to the same span identities;
`report FILE --output DIRECTORY` exports summaries and a timeline. Native capture loss,
local capacity loss and failed execution are separate observations. Traced timings
include instrumentation overhead and are not uninstrumented benchmark results.
`cargo observe bench PLAN --output DIRECTORY --runs N` rotates
disabled, wall, thread-CPU and native recording modes over paired batches, checks
outputs and reports action-level perturbation. Disabled retains compiled probes;
native harness duration includes export and must not be compared as action latency.
`PLECTRON_EXPERIMENT_OUTPUT=/absolute/new-directory cargo test -p plectron-observe --release
--test handoff experiment -- --ignored` compares platform-request and result-publication
wakeups independently. Set `PLECTRON_EXPERIMENT_SAMPLES` to override samples per batch.
`cargo observe handoff DIRECTORY/experiment.json --output NEW_DIRECTORY`
summarizes paired batches and observed handoffs. This in-process probe retains the
polling timeout and excludes native event-loop, IPC-client and rendering work; it
does not enable notification in the application. Runtime construction and warmup
are excluded; action timing includes execution, checkpoint, finish and result receipt.
`cargo observe ipc --baseline OLD_BINARY --internal INTERNAL_BINARY
--candidate NEW_BINARY --output NEW_DIRECTORY --rounds 3 --runs 40` measures actual
application control through Unix sockets. Admission ends at the eval acknowledgement;
action completion ends at the first control response reporting the finished job.
The client adds no polling sleep. Preserve request counts and all binary hashes;
keep unobserved timings separate from diagnostic exports and lifecycle CPU accounting.
Application composition explicitly supplies its platform notifier for queue notification.
Notifications follow successful publication and may coalesce; consumers inspect and
drain queues with a work bound instead of treating wakeups as message counts. Preserve
message/error delivery barriers while draining, and rearm readiness for remaining work.
Content result publication, Metal command completion, presentation-result publication
and IPC admission signal the same native event source.
`cargo observe frames --baseline OLD_BINARY --candidate NEW_BINARY
--apps /absolute/apps --output NEW_DIRECTORY --rounds 3 --runs 20` compares
native Metal frame receipts for repeatable cards width and layers opacity changes.
Completion requires the content job, changed revision/frame, matching displayed
scene/presented revision, and no pending or issued frame. Full page inspection is
included and timed separately. Use `--interval 5` to test a slower client-query cadence:
queries themselves can wake the event loop. Compare semantic scene/geometry signatures
and computation counters; this is not a pixel oracle or physical scanout measurement.
`cargo spec dispatch-measure --scenario chat --runs 20 --delays 0 4 8 12 16
--output NEW_DIRECTORY` runs each compute dispatch policy in fresh processes. Serial Metal,
CPU, overlapped Metal initialization, the production Metal executor and delayed CPU hedges
consume equivalent immutable passes. Reports retain winner counts, duplicate execution and artifact differences; their
endpoint excludes readback and physical presentation.
`cargo spec stream-measure --scenario chat --runs 20 --rounds 20 --lanes 1 2 4
--output NEW_DIRECTORY` compares warmed direct Metal and the production executor in fresh
processes. Each round submits the requested number of equivalent immutable passes before
waiting; preparation, readback, presentation and one warmup are excluded.
`cargo model ctl PID observe '{"type":"start","options":{}}'` starts an application
session; status, stop and discard use the same typed command entrypoint. Stop and
discard take `id`; stopped sessions admit no new intervals, including queued work
that has not begun. Already-open intervals may finish. Discard requires stopped
and zero open intervals; start never replaces retained evidence.
`cargo model observe PID SESSION OUTPUT.json` exports a snapshot through the binary
file channel. Export may be partial and does not stop recording. Task submission
carries explicit context across control/content threads and platform callbacks;
unrelated work must not inherit the receiver's ambient context. This does not
retroactively instrument work already running when a session starts.
Observation envelopes capture context, source thread and a monotonic timestamp;
receive intervals measure handler execution, with handoff latency reported separately
from capture to handler entry. Handoff includes any intervening sender work, blocking,
queueing and scheduling; it is not pure queue residency or CPU time, and must not be
added blindly to enclosing intervals. Only received envelopes produce receive records;
report completeness concerns recorded intervals, not complete workload coverage.
Publication and installation retain separate intervals. Promise jobs inherit the
context that enqueues runnable work, not a later checkpoint's ambient context.
Resource polling can merge several completions: preserve each callback's provenance
without assigning the merged render/publication to one arbitrary request. Until such
multi-cause attribution is modeled, detached worker polling leaves merged work
unattributed rather than borrowing the most recent task's context.
An interval's `parent` records causality; `enclosing` records the active execution
interval on the same thread and in the same observation session. Deferred work can
have different values. Self wall time subtracts execution children, not causal
descendants. Missing historical nesting falls back to parent with an explicit
legacy label; do not present those reports as newly verified exclusive accounting.
`cargo spec startup-measure --output NEW_DIRECTORY` owns fresh-process startup
observation; add `--application PATH --native-window` for the native application path.
Chromium observation runs through `cargo chromium measure`. Consult
their CLI options and executable assertions. Font asset generation and external shaping
oracles run through `cargo spec font-assets` and `cargo spec shape-oracle`; both retain
the upstream source identity, revision, executable version and content digests in their
evidence manifests.
Chromium observation is separate from the Plectron conformance suite and introduces
no compatibility requirement. Close every process or browser session the run owns.

`cargo model ctl PID query '{"type":"latest","unit":1}'` reads the latest published
frame record. Accepted asynchronous IPC jobs return an independent `record` ticket;
`query '{"type":"read","record":N}'` reads it and
`query '{"type":"wait","record":N,"timeout":2000}'` waits for its terminal state.
Timeout is milliseconds and expires the wait, not the task. Ordinary reads never
advance execution. The query service owns only a read capability; JS and forced page
inspection stay on their owner threads. Task completion and frame completion are
distinct records, not an inferred one-to-one association for concurrent changes.
Record identifiers are contiguous within one publication store, independent of engine
job/frame identifiers. Unknown, evicted, unavailable owner result, cancellation and
execution failure remain distinct. Retention bounds terminal history; latest frame
records remain separately retained for live units. Closing a query service does not
cancel execution. Closing an execution owner cancels its pending records.
`admitted`, `produced`, `visible`, `detected`, `seen` and `replied` are host-monotonic nanoseconds from
the publication store's epoch. Produced timestamps are optional producer-side result
construction times; visible is optional platform evidence that a frame drawable reached
physical presentation; detected marks owner publication, and replied marks query response
construction, not socket delivery. Visibility detection may update a completed frame record
without changing its completion detection time or page readiness. Content owner publication can include installation
and submission work after report receipt. Frame production includes drawable acquisition
and completion of the presentation copy; it is not a hardware GPU interval. Platforms
without physical presentation evidence leave visible absent. These intervals can overlap
and must not be added. A drawable callback without a presentation timestamp is a dropped
presentation. The graphics scheduler retains the same completed immutable frame and retries
its surface presentation until visibility succeeds or the existing presentation deadline
expires; it does not request a DOM, layout or scene recomputation and does not block content
completion or logical interaction while awaiting physical visibility.
`cargo observe passive --binary target/release/plectron --apps
apps --output NEW_DIRECTORY --rounds 3 --runs 20` checks cards and layers using
zero queries during a quiet interval, low/high-frequency reads and explicit waits.
Compare producer/owner times, retained records, scene signatures and computation counts.
Setup and post-completion full page checks are explicit; query-free does not mean
an application without an IPC server. Preserve timing differences from scheduling and
display pacing instead of claiming that decoupling makes every duration identical.
`cargo observe feel --binary target/release/plectron --project
apps/visual --output NEW_DIRECTORY --label slow --seconds 20` records a bounded manual
gesture session in a real macOS window. Run slow, fast and reversal gestures as separate
labels. `--drive forward` and `--drive reversal` provide deterministic Display-path
calibration without claiming native input behavior. The raw frame ledger retains
submission, backend completion, owner detection, drawable visibility and drops; the
accompanying observation report locates engine work.
Active cadence excludes adjacent admissions more than 100ms apart, but remains dependent
on human input cadence. Only interpret timing when `assessment.valid` is true; incomplete
physical visibility or an unsettled presentation invalidates the run. Treat each run as
diagnostic evidence rather than a repeatable benchmark or cross-engine comparison, and
close the owned application when recording ends.
