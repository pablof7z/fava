# fava-routing

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Descriptions are hand-written and preserved across updates. Re-exports appear
at their exported path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
| Kind | Item | Description |
| --- | --- | --- |
| Module | `fava_routing` |  |
| Enum | `fava_routing::CoverageState` |  |
| Enum variant | `fava_routing::CoverageState::Covered` |  |
| Public field | `fava_routing::CoverageState::Covered::0` |  |
| Enum variant | `fava_routing::CoverageState::SettledAbsent` |  |
| Enum variant | `fava_routing::CoverageState::Unresolved` |  |
| Struct | `fava_routing::PlannedRelay` |  |
| Public field | `fava_routing::PlannedRelay::reasons` |  |
| Public field | `fava_routing::PlannedRelay::session` |  |
| Public field | `fava_routing::PlannedRelay::targets` |  |
| Struct | `fava_routing::RouteContribution` |  |
| Public field | `fava_routing::RouteContribution::coverage` |  |
| Public field | `fava_routing::RouteContribution::destinations` |  |
| Public field | `fava_routing::RouteContribution::shortfalls` |  |
| Public field | `fava_routing::RouteContribution::unresolved` |  |
| Struct | `fava_routing::RouteDestination` |  |
| Method | `fava_routing::RouteDestination::new` |  |
| Public field | `fava_routing::RouteDestination::reason` |  |
| Method | `fava_routing::RouteDestination::router` |  |
| Public field | `fava_routing::RouteDestination::session` |  |
| Public field | `fava_routing::RouteDestination::targets` |  |
| Struct | `fava_routing::RoutePlan` |  |
| Public field | `fava_routing::RoutePlan::coverage` |  |
| Public field | `fava_routing::RoutePlan::destinations` |  |
| Method | `fava_routing::RoutePlan::explicit` |  |
| Method | `fava_routing::RoutePlan::from_contribution` |  |
| Public field | `fava_routing::RoutePlan::revision` |  |
| Method | `fava_routing::RoutePlan::settled` |  |
| Method | `fava_routing::RoutePlan::shortfall` |  |
| Public field | `fava_routing::RoutePlan::shortfalls` |  |
| Public field | `fava_routing::RoutePlan::unresolved` |  |
| Enum | `fava_routing::RouteRequest` |  |
| Enum variant | `fava_routing::RouteRequest::Read` |  |
| Public field | `fava_routing::RouteRequest::Read::0` |  |
| Enum variant | `fava_routing::RouteRequest::Write` |  |
| Public field | `fava_routing::RouteRequest::Write::0` |  |
| Method | `fava_routing::RouteRequest::access` |  |
| Method | `fava_routing::RouteRequest::event` |  |
| Method | `fava_routing::RouteRequest::is_read` |  |
| Method | `fava_routing::RouteRequest::is_write` |  |
| Method | `fava_routing::RouteRequest::targets` |  |
| Enum | `fava_routing::RouteTarget` |  |
| Enum variant | `fava_routing::RouteTarget::Author` |  |
| Public field | `fava_routing::RouteTarget::Author::0` |  |
| Enum variant | `fava_routing::RouteTarget::Recipient` |  |
| Public field | `fava_routing::RouteTarget::Recipient::0` |  |
| Enum variant | `fava_routing::RouteTarget::ReferencedEvent` |  |
| Public field | `fava_routing::RouteTarget::ReferencedEvent::0` |  |
| Enum variant | `fava_routing::RouteTarget::WholeRequest` |  |
| Trait | `fava_routing::Router` |  |
| Method | `fava_routing::Router::name` |  |
| Method | `fava_routing::Router::open` |  |
| Method | `fava_routing::Router::preview` |  |
| Enum | `fava_routing::RouterError` |  |
| Enum variant | `fava_routing::RouterError::Closed` |  |
| Enum variant | `fava_routing::RouterError::Refused` |  |
| Public field | `fava_routing::RouterError::Refused::0` |  |
| Trait | `fava_routing::RouterSession` |  |
| Method | `fava_routing::RouterSession::close` |  |
| Method | `fava_routing::RouterSession::current` |  |
| Method | `fava_routing::RouterSession::next_change` |  |
| Function | `fava_routing::open` |  |
| Function | `fava_routing::preview` |  |
<!-- END crate-readme-api inventory -->
