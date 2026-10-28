# Testing principles

Test the lowest layer that can establish the behavior: pure state/geometry/parser
tests, framework entity/event contexts, real component interactions, then native
application checks. Prefer a regression that fails for the reported defect over
setter tests or assertions that merely restate implementation details.

For interactive controls, cover activation, focus, disabled/read-only behavior,
controlled values, callback counts/order and retained identity. Test failures,
cancellation and replacement while asynchronous work is pending. Large-list
checks should exercise virtualization and stale registration cleanup rather than
claim performance from a correctness test.

Use the [Kit testing guide](../../crates/wgpui-kit/TESTING.md) for scoped IDs,
immutable observations, input simulation and bounded waits. Re-query after every
interaction. Do not add test-only state setters or infer unreported state.

Select real rendering explicitly when testing visible output, and use
[native validation](native-validation.md) for platform interactions. Respect
reduced motion and the actual clock used by an animation. Report portable,
rendering and native evidence separately, along with formatting, strict lint,
documentation checks and any remaining platform prerequisites.
