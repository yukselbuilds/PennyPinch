import type { ClientKind } from "./integrations";

/** Content-free observations used by the optional Overview recommendations. */
export type Recommendation =
  | {
    kind: "input_growth";
    basis: "local_estimate" | "provider_tokenizer";
    component: "message_input" | "whole_input";
    before_tokens: number;
    after_tokens: number;
    events: number;
  }
  | { kind: "slow_optimization"; p95_ms: number; requests: number }
  | {
    kind: "cache_share_drop";
    model: string;
    provider: string;
    previous_hits: number;
    previous_requests: number;
    recent_hits: number;
    recent_requests: number;
  }
  | { kind: "installation_stale"; client: ClientKind };
