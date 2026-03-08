# Danube Connectors Implementation Plan

## Goal

Refactor connector crates so they become thin integration adapters built on top of `danube-connect-core`. Connectors should be simple to configure, easy to maintain, and focused only on platform integration.

## Target Connector Shape

A connector crate should primarily contain:

- typed connector config specific to the external platform
- platform client setup
- transformation between generic core records and platform requests/responses
- adapter-specific validation that cannot live in core
- minimal startup/shutdown glue

A connector crate should **not** own:

- generic config loading framework
- generic batching engines
- flush timers
- health-check scheduling
- metrics bootstrap
- source stream-to-polling adaptation
- repeated schema/routing boilerplate

## Guiding Rules

- Keep connector code close to the external platform boundary.
- Push reusable logic downward into core if it is not platform-specific.
- Default configuration must be small and intent-oriented.
- Advanced configuration may exist, but only where the target platform genuinely requires it.
- Connector crates should consume `SinkRecord` and `SourceRecord` as the stable generic interface exposed by core.

## Migration Strategy

## 1. Normalize the public connector contract

Each connector should be split conceptually into:

- `config`
- `adapter`
- optional `transform`
- `main` / bootstrap

Where possible, avoid large all-in-one connector structs that also manage buffering, timing, and lifecycle policy.

## 2. Remove duplicated operational logic from connector crates

As core gains new abstractions, migrate connectors to them:

- replace custom `load()` / env override code with core config loader
- replace custom sink batching with core buffering
- replace custom source stream channel plumbing with core stream support
- rely on runtime health/metrics features from core

## 3. Adopt a simple-first configuration model

Each connector should define two usage tiers.

### Simple mode

Minimal fields for common use cases.

### Advanced mode

Optional extra settings for edge cases or performance tuning.

This must be reflected in both docs and config structures.

## Connector-specific Plans

## `source-mqtt`

### Keep in connector

- MQTT client setup
- subscription creation
- MQTT event conversion into `SourceRecord`
- MQTT-specific auth and connection parameters
- MQTT metadata extraction when enabled

### Move to core or simplify

- stream/channel lifecycle scaffolding
- batching of emitted `SourceRecord` values
- generic route handling
- schema association boilerplate
- generic config loading and env override framework

### UX simplification target

Default user config should look like a small set of routes:

- broker endpoint
- client identity
- `from` MQTT pattern
- `to` Danube topic
- optional QoS override

Advanced options like partition count, dispatch policy, and metadata controls should remain available but not front-loaded.

### Refactor note

If MQTT QoS implies delivery behavior, expose that as a documented default, not as hard-coded hidden policy. Users should be able to override it explicitly.

## `sink-qdrant`

### Keep in connector

- Qdrant client setup
- collection existence checks / creation
- translation of normalized records into Qdrant points
- Qdrant write execution
- Qdrant-specific validation such as vector dimension and distance metric compatibility

### Move to core or simplify

- batch buffering and timeout flush orchestration
- generic topic route plumbing
- generic consumer config repetition
- config loading framework

### UX simplification target

Default user config should support a very small setup for a single collection:

- Danube topic
- subscription
- Qdrant URL
- collection name
- vector dimension

Everything else should have sensible defaults.

### Message contract direction

The connector may keep a default vector-envelope expectation, but it should evolve toward configurable extraction over the normalized payload provided by core. The connector should not require a deeply opinionated message shape unless the user opts into that mode.

## `sink-deltalake`

### Keep in connector

- Delta Lake table open/create operations
- backend-specific storage option translation
- conversion from normalized records into Arrow/Delta writes
- Delta-specific write execution and table refresh

### Move to core or simplify

- generic batching / periodic flush scheduling
- config loading framework
- route boilerplate
- generic schema-aware projection helpers if they are broadly reusable

### UX simplification target

Default user config should support a simple append mode such as:

- topic
- subscription
- table path
- storage connection basics
- optional schema subject

Advanced explicit field mapping should remain available, but it should be documented as advanced mode rather than the default mental model.

### Refactor note

Avoid exposing config knobs that are not fully wired, such as partial write-mode support. Remove or defer options until behavior is complete.

## Cross-Connector Cleanup

## 1. Standardize config structure

Every connector should converge on the same structure:

- shared root/core config
- one platform section
- one route list or mapping list
- the same override semantics

The names do not need to be identical everywhere, but the user experience should feel consistent.

## 2. Standardize route vocabulary

Prefer a shared mental model across connectors:

- `from`
- `to`
- `subscription`
- optional batch policy
- optional schema expectation
- optional metadata policy

Connector-specific fields can be nested under each route when necessary.

## 3. Reduce example complexity

Current examples are useful as reference material, but they are too dense as onboarding material.

For each connector, provide:

- one minimal example
- one advanced example
- one production example

The minimal example should fit on one screen.

## 4. Audit misleading configuration surface

For each connector, remove, defer, or fully implement options that are only partially wired.

This includes reviewing:

- options that are documented but not applied
- options that exist only for future plans
- deprecated config structures still visible to users

## 5. Make connector docs architecture-driven

Each connector README should explain:

- what the connector is responsible for
- what core is responsible for
- what the message contract is
- what the simplest valid config looks like
- which advanced features are optional

## Recommended Delivery Order

## Phase 1: Prepare connectors for core migration

- create thin `adapter` boundaries inside each connector crate
- isolate batching/config/runtime code so it can be removed cleanly
- document the intended minimal config path for each connector

## Phase 2: Migrate first wave connectors

Use the current target set as the proving ground:

- `source-mqtt`
- `sink-qdrant`
- `sink-deltalake`

For each one:

- replace config loader with core loader
- replace runtime-heavy code with core abstractions
- simplify public config and examples

## Phase 3: Extract reusable patterns from migration

After the first wave, codify:

- route naming conventions
- minimal example format

## Success Criteria

- connector crates shrink in code size and responsibility
- connector READMEs become shorter and easier to understand
- minimal configs are truly minimal
- operational behavior becomes consistent across connectors
- connectors no longer re-implement core runtime concerns
- advanced functionality remains possible without dominating default configuration

## Non-Goals

- removing all advanced connector capabilities
- forcing all connectors into the same target-specific config shape
- pushing external platform behavior into core
- performing a breaking migration for every connector in one release
