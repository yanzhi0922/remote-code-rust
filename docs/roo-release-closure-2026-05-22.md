# Roo Release Closure Record

Date: 2026-05-22

This record closes the Roo permission, token usage, and MCP documentation items
that were previously listed as release risks in `docs/requirements.md`.

## Permission

The Roo adapter maps tool approval, follow-up, completion feedback, API retry,
auto-approval limit, and mistake-limit requests into the shared permission
surface and routes GUI decisions back through `resolve_permission()`.

Latest evidence:

- `.release-evidence/20260521-225254/logs/14-1-base-gates.log`
- `rc-roo-adapter` tests named `tool_approval_required_maps_to_permission_request`
  and `resolve_permission_*`

## Provider Usage Boundary

Roo release accounting does not compare a character-count estimate with provider
usage at the adapter boundary. The Roo task loop accumulates parsed provider
stream usage fields into `result.token_usage`; the adapter uses that result for
completed usage and context usage events.

Boundary record:

| Boundary | Source | Recorded deviation |
| --- | --- | --- |
| Provider stream usage to Roo task result | Parsed `input_tokens`, `output_tokens`, cache usage, and cost from Roo provider stream events | No estimate substitution in this path |
| Roo task result to adapter completed usage | `result.token_usage` | 0 field-level transformation for input/output/cache counts |
| Roo token usage event to GUI context usage | Roo `TokenUsageUpdated` / `TaskTokenUsageUpdated` | Input plus output is mapped to `used`; context window is mapped to `total` |

Latest evidence:

- `crates/roo/roo-task/src/agent_loop.rs`
- `crates/adapters/rc-roo-adapter/src/lib.rs`
- `.release-evidence/20260521-225254/logs/14-1-base-gates.log` tests named
  `token_usage_updated_maps_to_context_usage`,
  `task_token_usage_updated_maps_to_context_usage`, and
  `context_budget_evaluated_maps_tokens`
- `.release-evidence/20260521-225254/logs/provider-*.log` provider smoke PASS

Raw provider billing exports and secret-bearing provider response logs must stay
outside Git-tracked release notes and follow the redaction policy.

## MCP

Roo native MCP handler registration and MCP tool/resource failure boundaries are
covered by Roo and adapter tests. The release MCP matrix separately verifies
discovery and calls for MiniMax, context7, sequentialthinking, memory, and
puppeteer.

Latest evidence:

- `.release-evidence/20260521-225254/logs/14-1-base-gates.log`
- `.release-evidence/20260521-225254/logs/mcp-*.log`
- `rc-roo-adapter` test `build_dispatcher_registers_mcp_handlers`
- `roo_tools_mcp` tool/resource tests
