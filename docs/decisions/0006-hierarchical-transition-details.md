# 0006. Details of hierarchical transitions left open by the spec

Status: accepted
Date: 2026-11-12
Spec: 3.4, 5.2, 5.3, 6.3

## Context

Writing the `Tree` test machine by hand (`crates/estado-core/tests/support/tree/`, tests in `tests/hierarchy.rs`) as the macro would generate it raised three questions the spec does not answer.

## Decision

1. **`reenter!()` keeps the state's own data.** Table 5.2 says `reenter!()` exits and enters `H` and its initials, but `reenter!()` carries no data to rebuild `H`. The state's own fields stay as they are; its substates restart from their `#[initial]`. To rebuild the data, write `goto!(H { .. })` instead, which has the same exit/entry order.
2. **A self-transition changes the configuration only if the active states differ afterwards.** `goto!(H)` and `reenter!()` on an atomic state, or on a compound already in its initial substates, report `Transitioned { changed: false }`, so deferred events are not requeued (spec 5.4 requeues only when the configuration changes). On a compound that was in another substate they report `changed: true`. Generated code compares the active leaf ids before and after.
3. **A local transition to an ancestor enters its initial, never its history.** The "(or history)" of table 5.2 row 3 applies only with `goto!(T, history)`. The ancestor does not exit, so its history would not have been recorded by this transition anyway.

Deep history combined with a constructor is spec errata 5.

## Consequences

- The macro generates `reenter!()` as in-place exits and entries on the live state, without taking it by value.
- Tests `ac_01_reenter_restarts_substates_and_keeps_own_data` and `ac_01_goto_the_handler_state_exits_and_enters_it` pin these rules.
