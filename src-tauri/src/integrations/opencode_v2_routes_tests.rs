use super::opencode_routes::{
    OpenCodeDefaultRouteCoverage, OpenCodeProtocol, OpenCodeRouteAnalysis,
    OpenCodeRouteAnalysisError, OpenCodeRouteBefore, OpenCodeRuntimeEffective,
};
use super::opencode_v2_routes::analyze_opencode_v2_routes;
use serde_json::{json, Map, Value};

const ENDPOINT: &str = "http://127.0.0.1:45123/v1";
const CANONICAL: &str = "https://api.openai.com/v1";
const OPENAI_PACKAGE: &str = "@opencode/ai/providers/openai";
const COMPATIBLE_PACKAGE: &str = "@opencode/ai/providers/openai-compatible";

fn analyze(value: Value) -> Result<OpenCodeRouteAnalysis, OpenCodeRouteAnalysisError> {
    analyze_opencode_v2_routes(value.as_object().expect("root object"), ENDPOINT)
}

fn provider(package: &str, base_url: &str) -> Value {
    json!({
        "package": package,
        "settings": { "baseURL": base_url },
        "models": { "catalog-name": { "modelID": "unfiltered-wire-model" } }
    })
}

fn route<'a>(
    result: &'a OpenCodeRouteAnalysis,
    key: &str,
) -> &'a super::opencode_routes::OpenCodeRoute {
    result
        .routes
        .iter()
        .find(|route| route.provider_key == key)
        .expect("expected route")
}

#[test]
fn inventories_v2_aliases_protocols_and_model_references_without_mutating_input() {
    let input = json!({
        "model": {
            "providerID": "team-compatible",
            "model": "arbitrary/catalog/wire-name",
            "variant": "reasoning"
        },
        "providers": {
            "team-compatible": {
                "package": COMPATIBLE_PACKAGE,
                "settings": { "baseURL": CANONICAL, "apiKey": "secret-canary" },
                "models": { "catalog-name": {
                    "modelID": "wire-id-that-is-not-filtered",
                    "package": COMPATIBLE_PACKAGE,
                    "variants": [{ "id": "reasoning", "settings": { "reasoningEffort": "high" } }]
                } }
            },
            "responses-alias": {
                "package": OPENAI_PACKAGE,
                "settings": { "baseURL": ENDPOINT },
                "models": { "another-name": { "modelID": "another-wire-id" } }
            }
        },
        "agents": {
            "build": { "model": "responses-alias/any-model#reasoning", "request": { "headers": {}, "body": {} } },
            "review": { "model": { "providerID": "team-compatible", "model": "wire/model", "variant": "reasoning" } }
        },
        "policies": [
            { "action": "provider.use", "resource": "*", "effect": "deny" },
            { "action": "provider.use", "resource": "team-compatible", "effect": "allow" },
            { "action": "integration.use", "resource": "team-compatible#fixture", "effect": "allow" }
        ]
    });
    let original = input.clone();
    let result = analyze(input.clone()).expect("supported V2 routes");

    assert_eq!(input, original);
    assert_eq!(result.routes.len(), 2);
    assert_eq!(
        route(&result, "responses-alias").protocol,
        OpenCodeProtocol::Responses
    );
    assert_eq!(
        route(&result, "responses-alias").before,
        OpenCodeRouteBefore::SameStableEndpoint
    );
    assert_eq!(
        route(&result, "team-compatible").protocol,
        OpenCodeProtocol::ChatCompletions
    );
    assert_eq!(
        route(&result, "team-compatible").before,
        OpenCodeRouteBefore::CanonicalOpenAiBase
    );
    assert_eq!(result.coverage.eligible_count, 2);
    assert_eq!(result.coverage.responses_count, 1);
    assert_eq!(result.coverage.chat_completions_count, 1);
    assert_eq!(result.coverage.excluded_count, 0);
    assert!(
        result.coverage.partial,
        "policy evaluation is not part of route analysis"
    );
    assert_eq!(
        result.coverage.runtime_effective,
        OpenCodeRuntimeEffective::Unknown
    );
    assert_eq!(
        result.coverage.configured_default_route,
        OpenCodeDefaultRouteCoverage::Covered
    );
    let debug = format!("{result:?}");
    assert!(!debug.contains("team-compatible"));
    assert!(!debug.contains("wire-id-that-is-not-filtered"));
    assert!(!debug.contains("secret-canary"));
    let coverage = serde_json::to_string(&result.coverage).expect("coverage is serializable");
    assert!(!coverage.contains("team-compatible"));
    assert!(!coverage.contains("secret-canary"));
    assert!(!coverage.contains(CANONICAL));
    assert!(!coverage.contains(ENDPOINT));
}

#[test]
fn builtin_openai_is_only_inventoried_when_explicit_and_default_coverage_is_configured_only() {
    let empty = analyze(json!({})).expect("empty V2 config is valid");
    assert!(empty.routes.is_empty());
    assert_eq!(empty.coverage.eligible_count, 0);
    assert_eq!(
        empty.coverage.configured_default_route,
        OpenCodeDefaultRouteCoverage::Unknown
    );

    let referenced = analyze(json!({ "model": "openai/arbitrary-model#reasoning" }))
        .expect("built-in reference alone must not activate a provider");
    assert_eq!(referenced.coverage.excluded_count, 1);
    assert!(referenced.routes.is_empty());
    assert_eq!(
        referenced.coverage.configured_default_route,
        OpenCodeDefaultRouteCoverage::Excluded
    );

    let configured = analyze(
        json!({ "model": "openai/arbitrary-model#reasoning", "providers": { "openai": {} } }),
    )
    .expect("already configured built-in provider");
    assert_eq!(configured.routes.len(), 1);
    assert_eq!(
        route(&configured, "openai").protocol,
        OpenCodeProtocol::Responses
    );
    assert_eq!(
        route(&configured, "openai").before,
        OpenCodeRouteBefore::Missing
    );
    assert_eq!(
        configured.coverage.configured_default_route,
        OpenCodeDefaultRouteCoverage::Covered
    );

    let object_reference = analyze(json!({
        "agents": { "builder": { "model": { "providerID": "openai", "model": "unlisted-wire-id" } } },
        "providers": {"openai": {}}
    }))
    .expect("structured agent model reference");
    assert_eq!(object_reference.routes.len(), 1);
    assert_eq!(
        object_reference.coverage.configured_default_route,
        OpenCodeDefaultRouteCoverage::Unknown
    );

    let plugin_free =
        analyze(json!({ "plugins": [], "model": "openai/m", "providers": {"openai": {}} }))
            .expect("empty native plugin list does not add plugin effects");
    assert_eq!(plugin_free.coverage.eligible_count, 1);

    let unconfigured_other = analyze(json!({ "model": "azure/any-model" }))
        .expect("unconfigured non-OpenAI provider is not inferred");
    assert_eq!(unconfigured_other.coverage.eligible_count, 0);
    assert_eq!(unconfigured_other.coverage.excluded_count, 1);
    assert_eq!(
        unconfigured_other.coverage.configured_default_route,
        OpenCodeDefaultRouteCoverage::Excluded
    );

    let excluded_default = analyze(json!({
        "model": "custom/model",
        "providers": { "custom": { "package": "other-sdk", "settings": { "baseURL": CANONICAL } } }
    }))
    .expect("unsupported default remains visible as excluded");
    assert_eq!(excluded_default.coverage.eligible_count, 0);
    assert_eq!(excluded_default.coverage.excluded_count, 1);
    assert_eq!(
        excluded_default.coverage.configured_default_route,
        OpenCodeDefaultRouteCoverage::Excluded
    );
}

#[test]
fn accepts_only_exact_openai_packages_and_known_endpoint_states() {
    let result = analyze(json!({
        "providers": {
            "openai": { "settings": { "apiKey": "secret-canary" } },
            "alias-responses": provider(OPENAI_PACKAGE, CANONICAL),
            "alias-chat": provider(COMPATIBLE_PACKAGE, ENDPOINT),
            "alias-other-host": provider(COMPATIBLE_PACKAGE, "https://gateway.invalid/v1"),
            "alias-other-package": provider("some-sdk", CANONICAL),
            "custom-without-base": { "package": COMPATIBLE_PACKAGE, "models": { "m": {} } }
        }
    }))
    .expect("unsupported endpoints become exclusions");

    assert_eq!(result.coverage.eligible_count, 3);
    assert_eq!(result.coverage.excluded_count, 3);
    assert_eq!(
        route(&result, "openai").before,
        OpenCodeRouteBefore::Missing
    );
    assert_eq!(
        route(&result, "alias-responses").before,
        OpenCodeRouteBefore::CanonicalOpenAiBase
    );
    assert_eq!(
        route(&result, "alias-responses").protocol,
        OpenCodeProtocol::Responses
    );
    assert_eq!(
        route(&result, "alias-chat").before,
        OpenCodeRouteBefore::SameStableEndpoint
    );
    assert_eq!(
        route(&result, "alias-chat").protocol,
        OpenCodeProtocol::ChatCompletions
    );
    assert!(result.coverage.partial);

    let configured_builtin = analyze(json!({
        "providers": {
            "openai": {
                "settings": { "baseURL": CANONICAL },
                "models": { "m": { "package": OPENAI_PACKAGE } }
            }
        },
        "model": "openai/m"
    }))
    .expect("unmodified built-in OpenAI package remains supported");
    assert_eq!(
        route(&configured_builtin, "openai").before,
        OpenCodeRouteBefore::CanonicalOpenAiBase
    );

    let replaced_builtin = analyze(json!({
        "model": "openai/m",
        "providers": { "openai": provider(COMPATIBLE_PACKAGE, CANONICAL) }
    }))
    .expect("a changed built-in package is excluded");
    assert_eq!(replaced_builtin.coverage.eligible_count, 0);
    assert_eq!(replaced_builtin.coverage.excluded_count, 1);
}

#[test]
fn pinned_builtin_provider_ids_and_route_overrides_are_excluded_without_blocking_safe_aliases() {
    let result = analyze(json!({
        "providers": {
            "safe": provider(COMPATIBLE_PACKAGE, CANONICAL),
            "xai": provider(COMPATIBLE_PACKAGE, CANONICAL),
            "lmstudio": provider(OPENAI_PACKAGE, CANONICAL),
            "gateway": provider(COMPATIBLE_PACKAGE, "https://gateway.invalid/v1"),
            "wrong-package": provider("third-party-sdk", CANONICAL),
            "model-package": {
                "package": COMPATIBLE_PACKAGE,
                "settings": { "baseURL": CANONICAL },
                "models": { "m": { "package": "third-party-sdk" } }
            },
            "nested-settings": {
                "package": COMPATIBLE_PACKAGE,
                "settings": { "baseURL": CANONICAL, "extra": { "transport": { "url": "https://other.invalid" } } },
                "models": { "m": {} }
            },
            "model-type": {
                "package": COMPATIBLE_PACKAGE,
                "settings": { "baseURL": CANONICAL },
                "models": { "m": { "type": "image" } }
            },
            "model-body": {
                "package": COMPATIBLE_PACKAGE,
                "settings": { "baseURL": CANONICAL },
                "models": { "m": { "body": {} } }
            },
            "variant-override": {
                "package": COMPATIBLE_PACKAGE,
                "settings": { "baseURL": CANONICAL },
                "models": { "m": { "variants": [{ "id": "v", "settings": { "transport": { "endpoint": "https://other.invalid" } } }] } }
            },
            "provider-headers": {
                "package": COMPATIBLE_PACKAGE,
                "settings": { "baseURL": CANONICAL },
                "headers": { "x-route": "changed" },
                "models": { "m": {} }
            },
            "provider-body": {
                "package": COMPATIBLE_PACKAGE,
                "settings": { "baseURL": CANONICAL },
                "body": {},
                "models": { "m": {} }
            },
            "provider-canonical": {
                "canonical": "openai",
                "package": COMPATIBLE_PACKAGE,
                "settings": { "baseURL": CANONICAL },
                "models": { "m": {} }
            }
        }
    }))
    .expect("route-changing providers are a partial inventory");

    assert_eq!(result.coverage.eligible_count, 1);
    assert_eq!(result.coverage.excluded_count, 12);
    assert_eq!(
        route(&result, "safe").protocol,
        OpenCodeProtocol::ChatCompletions
    );
    assert!(result.coverage.partial);
}

#[test]
fn rejects_legacy_roots_plugins_and_agent_request_route_overrides_as_unsupported() {
    for key in [
        "provider",
        "agent",
        "mode",
        "enabled_providers",
        "disabled_providers",
        "small_model",
        "plugin",
        "tools",
        "permission",
        "experimental",
        "command",
    ] {
        let mut root = Map::new();
        root.insert(key.to_owned(), json!({}));
        assert_eq!(
            analyze_opencode_v2_routes(&root, ENDPOINT),
            Err(OpenCodeRouteAnalysisError::UnsupportedConfig),
            "legacy root key {key}"
        );
    }
    assert_eq!(
        analyze(json!({ "plugins": ["local-plugin"] })),
        Err(OpenCodeRouteAnalysisError::UnsupportedConfig)
    );
    assert_eq!(
        analyze(
            json!({ "agents": { "builder": { "model": "openai/m", "request": { "headers": { "x-route": "custom" } } } } })
        ),
        Err(OpenCodeRouteAnalysisError::UnsupportedConfig)
    );
    assert_eq!(
        analyze(
            json!({ "agents": { "builder": { "request": { "options": { "fetch": "custom" } } } } })
        ),
        Err(OpenCodeRouteAnalysisError::UnsupportedConfig)
    );
    assert_eq!(
        analyze(json!({ "agents": { "builder": { "request": { "body": { "max_tokens": 1 } } } } })),
        Err(OpenCodeRouteAnalysisError::UnsupportedConfig)
    );
    let empty_request = analyze(json!({
        "model": "openai/m",
        "agents": { "builder": { "model": "openai/agent-model", "request": { "headers": {}, "body": {} } } },
        "providers": {"openai": {}}
    }))
    .expect("empty agent request overlays do not alter route");
    assert_eq!(empty_request.coverage.eligible_count, 1);
}

#[test]
fn validates_policy_shape_and_marks_policy_coverage_unknown() {
    let with_policy = analyze(json!({
        "model": "openai/m",
        "providers": {"openai": {}},
        "policies": [{ "action": "provider.use", "resource": "*", "effect": "deny" }]
    }))
    .expect("well-shaped policy is observed, not evaluated");
    assert_eq!(
        with_policy.coverage.configured_default_route,
        OpenCodeDefaultRouteCoverage::Covered
    );
    assert!(with_policy.coverage.partial);

    for malformed in [
        json!({ "policies": {} }),
        json!({ "policies": [{ "action": "unknown.action", "resource": "*", "effect": "deny" }] }),
        json!({ "policies": [{ "action": "provider.use", "resource": 4, "effect": "deny" }] }),
        json!({ "policies": [{ "action": "provider.use", "resource": "*", "effect": "ask" }] }),
    ] {
        assert_eq!(
            analyze(malformed),
            Err(OpenCodeRouteAnalysisError::InvalidSchema)
        );
    }
}

#[test]
fn malformed_root_provider_model_variant_selector_and_agent_shapes_fail_closed() {
    let malformed = [
        json!({ "providers": [] }),
        json!({ "providers": { "safe": "not-an-object" } }),
        json!({ "providers": { "safe": { "models": [] } } }),
        json!({ "providers": { "safe": { "models": { "m": { "modelID": 8 } } } } }),
        json!({ "providers": { "safe": { "models": { "m": { "type": 8 } } } } }),
        json!({ "providers": { "safe": { "models": { "m": { "variants": {} } } } } }),
        json!({ "providers": { "safe": { "models": { "m": { "variants": [{ "id": 8 }] } } } } }),
        json!({ "model": "openai/" }),
        json!({ "model": "openai/m#" }),
        json!({ "model": { "providerID": "openai", "model": "m", "other": true } }),
        json!({ "agents": [] }),
        json!({ "agents": { "builder": { "model": 3 } } }),
        json!({ "agents": { "builder": { "request": { "headers": { "x": 1 } } } } }),
    ];
    for input in malformed {
        assert_eq!(
            analyze(input),
            Err(OpenCodeRouteAnalysisError::InvalidSchema)
        );
    }
}

#[test]
fn enforces_provider_model_agent_policy_variant_selector_and_nesting_bounds() {
    let providers_33 = (0..33)
        .map(|index| (format!("p{index}"), json!({})))
        .collect::<Map<String, Value>>();
    assert_eq!(
        analyze(json!({ "providers": providers_33 })),
        Err(OpenCodeRouteAnalysisError::LimitExceeded)
    );

    let providers_32 = (0..32)
        .map(|index| (format!("p{index}"), json!({})))
        .collect::<Map<String, Value>>();
    assert_eq!(
        analyze(json!({ "model": "unlisted/m", "providers": providers_32 })),
        Err(OpenCodeRouteAnalysisError::LimitExceeded)
    );

    let model_set = (0..129)
        .map(|index| (format!("m{index}"), json!({})))
        .collect::<Map<String, Value>>();
    assert_eq!(
        analyze(
            json!({ "providers": { "custom": { "package": COMPATIBLE_PACKAGE, "settings": { "baseURL": CANONICAL }, "models": model_set } } })
        ),
        Err(OpenCodeRouteAnalysisError::LimitExceeded)
    );

    let variants = (0..33)
        .map(|index| json!({ "id": format!("v{index}") }))
        .collect::<Vec<_>>();
    assert_eq!(
        analyze(
            json!({ "providers": { "custom": { "package": COMPATIBLE_PACKAGE, "settings": { "baseURL": CANONICAL }, "models": { "m": { "variants": variants } } } } })
        ),
        Err(OpenCodeRouteAnalysisError::LimitExceeded)
    );

    let agents = (0..33)
        .map(|index| (format!("a{index}"), json!({})))
        .collect::<Map<String, Value>>();
    assert_eq!(
        analyze(json!({ "agents": agents })),
        Err(OpenCodeRouteAnalysisError::LimitExceeded)
    );

    let policies = (0..129)
        .map(|_| json!({ "action": "provider.use", "resource": "*", "effect": "allow" }))
        .collect::<Vec<_>>();
    assert_eq!(
        analyze(json!({ "policies": policies })),
        Err(OpenCodeRouteAnalysisError::LimitExceeded)
    );

    let deep = (0..10).fold(json!("value"), |child, _| json!({ "nested": child }));
    assert_eq!(
        analyze(
            json!({ "providers": { "custom": { "package": COMPATIBLE_PACKAGE, "settings": { "baseURL": CANONICAL, "ordinary": deep }, "models": { "m": {} } } } })
        ),
        Err(OpenCodeRouteAnalysisError::LimitExceeded)
    );

    let oversized_selector = format!("openai/{}", "m".repeat(512));
    assert_eq!(
        analyze(json!({ "model": oversized_selector })),
        Err(OpenCodeRouteAnalysisError::LimitExceeded)
    );

    let long_key = "k".repeat(129);
    assert_eq!(
        analyze(json!({ "providers": { (long_key): {} } })),
        Err(OpenCodeRouteAnalysisError::LimitExceeded)
    );
}

#[test]
fn enforces_the_eligible_route_limit_without_truncating() {
    let providers = (0..17)
        .map(|index| {
            (
                format!("alias-{index}"),
                provider(COMPATIBLE_PACKAGE, CANONICAL),
            )
        })
        .collect::<Map<String, Value>>();
    assert_eq!(
        analyze(json!({ "providers": providers })),
        Err(OpenCodeRouteAnalysisError::LimitExceeded)
    );
}
