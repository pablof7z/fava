# 0058: current-account observation synchronization

## problem

`Observation::changed` reports the next unread immutable revision. After an
account switch that revision may be relay or source evidence produced by the
prior account before the query owner publishes the replacement generation. An
application cannot know when `$currentPubkey` has converged without inspecting
selected keys and event authors, which duplicates Fava's generation authority
and can hide stale-delivery defects.

## decision

`Observation::synchronize_current_account(timeout)` waits inside Fava until the
observation has delivered a snapshot under the exact current selection tuple.
It returns `Ok(None)` on the caller's bound and leaves the observation open,
matching `wait_until`. Literal observations have no account generation and
return their current snapshot immediately.

The reactive owner publishes an internal `(selection, revision, snapshot)`
watch value in the same session and registry critical sections as public latest
snapshot delivery. Synchronization subscribes to session changes before reading
the target and admits a result only through `Session::if_current_account`, so a
concurrent switch cannot make a retired delivery current.

## evidence

The public rapid-switch test asks for synchronization after A→B→A→B→C and
receives only C. Existing source-open, diagnostic, provisional-demand, close,
wire, and stable-identity regressions remain unchanged. Removing the exact tuple
comparison makes the synchronization test return a prior generation.
