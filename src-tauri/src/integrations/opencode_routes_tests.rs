use super::opencode_routes::{
    analyze_opencode_v1_routes, OpenCodeProtocol, OpenCodeRouteAnalysisError, OpenCodeRouteBefore,
};
use serde_json::{json, Map, Value};

const ENDPOINT: &str = "http://127.0.0.1:45123/v1";
const CANONICAL: &str = "https://api.openai.com/v1";

fn analyze(
    value: Value,
) -> Result<super::opencode_routes::OpenCodeRouteAnalysis, OpenCodeRouteAnalysisError> {
    analyze_opencode_v1_routes(value.as_object().expect("root object"), ENDPOINT)
}

fn custom_provider(base_url: &str, npm: &str) -> Value {
    json!({
        "npm": npm,
        "options": { "baseURL": base_url },
        "models": { "wire-id": {} }
    })
}

#[test]
fn inventories_multiple_supported_aliases_without_exposing_identifiers_or_model_values() {
    let result = analyze(json!({
        "model": "openai/selected-wire-model",
        "small_model": "local/another-wire-model",
        "provider": {
            "local": custom_provider(CANONICAL, "@ai-sdk/openai-compatible"),
            "openai": { "options": { "baseURL": ENDPOINT } },
            "anthropic": { "npm": "@ai-sdk/anthropic" }
        },
        "agent": { "build": { "model": "local/agent-wire-model" } }
    }))
    .expect("supported routes should be inventoried");

    assert_eq!(result.routes.len(), 2);
    assert_eq!(result.routes[0].provider_key, "local");
    assert_eq!(result.routes[0].protocol, OpenCodeProtocol::ChatCompletions);
    assert_eq!(
        result.routes[0].before,
        OpenCodeRouteBefore::CanonicalOpenAiBase
    );
    assert_eq!(result.routes[1].provider_key, "openai");
    assert_eq!(result.routes[1].protocol, OpenCodeProtocol::Responses);
    assert_eq!(
        result.routes[1].before,
        OpenCodeRouteBefore::SameStableEndpoint
    );
    assert_eq!(result.coverage.eligible_count, 2);
    assert_eq!(result.coverage.excluded_count, 1);
    assert_eq!(result.coverage.already_same_count, 1);
    assert_eq!(result.coverage.responses_count, 1);
    assert_eq!(result.coverage.chat_completions_count, 1);
    assert_eq!(
        result.coverage.configured_default_route,
        super::opencode_routes::OpenCodeDefaultRouteCoverage::Covered
    );
    assert!(result.coverage.partial);
    let serialized = serde_json::to_string(&result.coverage).expect("coverage serialization");
    assert!(serialized.contains("\"runtime_effective\":\"unknown\""));
    assert!(!serialized.contains("local"));
    assert!(!serialized.contains("selected-wire-model"));
    assert!(!serialized.contains(CANONICAL));
    assert!(!serialized.contains(ENDPOINT));
}

#[test]
fn builtin_openai_is_included_only_when_explicitly_referenced_enabled_or_configured() {
    let empty = analyze(json!({})).expect("empty valid V1 root");
    assert!(empty.routes.is_empty());
    assert_eq!(
        empty.coverage.configured_default_route,
        super::opencode_routes::OpenCodeDefaultRouteCoverage::Unknown
    );

    let referenced = analyze(json!({ "model": "openai/arbitrary-model-id" }))
        .expect("explicit builtin reference");
    assert_eq!(referenced.routes.len(), 1);
    assert_eq!(referenced.routes[0].before, OpenCodeRouteBefore::Missing);
    assert_eq!(
        referenced.coverage.configured_default_route,
        super::opencode_routes::OpenCodeDefaultRouteCoverage::Covered
    );

    let enabled =
        analyze(json!({ "enabled_providers": ["openai"] })).expect("explicit enabled builtin");
    assert_eq!(enabled.routes.len(), 1);

    let configured =
        analyze(json!({ "provider": { "openai": {} } })).expect("explicit configured builtin");
    assert_eq!(configured.routes.len(), 1);
}

#[test]
fn provider_enablement_and_disabled_lists_exclude_routes_without_rewriting_them() {
    let result = analyze(json!({
        "enabled_providers": ["first"],
        "disabled_providers": ["second"],
        "model": "first/model-id",
        "provider": {
            "first": custom_provider(CANONICAL, "@ai-sdk/openai-compatible"),
            "second": custom_provider(CANONICAL, "@ai-sdk/openai-compatible"),
            "third": custom_provider(CANONICAL, "@ai-sdk/openai-compatible")
        }
    }))
    .expect("valid provider filters");

    assert_eq!(result.routes.len(), 1);
    assert_eq!(result.routes[0].provider_key, "first");
    assert_eq!(result.coverage.excluded_count, 2);
    assert!(result.coverage.partial);
    assert_eq!(
        result.coverage.configured_default_route,
        super::opencode_routes::OpenCodeDefaultRouteCoverage::Covered
    );

    let excluded_default = analyze(json!({
        "model": "unsupported/model-id",
        "provider": {
            "openai": { "options": { "baseURL": CANONICAL } },
            "unsupported": { "npm": "custom-sdk", "options": { "baseURL": CANONICAL } }
        }
    }))
    .expect("unsupported default remains an explicit partial route");
    assert_eq!(
        excluded_default.coverage.configured_default_route,
        super::opencode_routes::OpenCodeDefaultRouteCoverage::Excluded
    );
}

#[test]
fn custom_alias_requires_explicit_compatible_package_canonical_url_and_safe_models() {
    let no_base = json!({
        "provider": { "custom": { "npm": "@ai-sdk/openai-compatible", "options": { "apiKey": "synthetic-secret" }, "models": { "m": {} } } }
    });
    let wrong_host = json!({
        "provider": { "custom": custom_provider("https://gateway.example/v1", "@ai-sdk/openai-compatible") }
    });
    let implicit_package = json!({
        "provider": { "custom": { "options": { "baseURL": CANONICAL }, "models": { "m": {} } } }
    });
    let wrong_package = json!({
        "provider": { "custom": custom_provider(CANONICAL, "custom-sdk") }
    });
    let empty_models = json!({
        "provider": { "custom": { "npm": "@ai-sdk/openai-compatible", "options": { "baseURL": CANONICAL } } }
    });

    for fixture in [
        no_base,
        wrong_host,
        implicit_package,
        wrong_package,
        empty_models,
    ] {
        let result = analyze(fixture).expect("unsupported routes are partial, not malformed");
        assert!(result.routes.is_empty());
        assert_eq!(result.coverage.excluded_count, 1);
        assert!(result.coverage.partial);
    }
}

#[test]
fn route_changing_provider_and_model_overrides_are_excluded() {
    let fixtures = [
        json!({ "provider": { "xai": custom_provider(CANONICAL, "@ai-sdk/openai-compatible") } }),
        json!({ "provider": { "azure": custom_provider(CANONICAL, "@ai-sdk/openai-compatible") } }),
        json!({ "provider": { "local": { "npm": "@ai-sdk/openai-compatible", "options": { "baseURL": CANONICAL, "endpoint": "https://other.example" }, "models": { "m": {} } } } }),
        json!({ "provider": { "local": { "npm": "@ai-sdk/openai-compatible", "options": { "baseURL": CANONICAL, "headers": { "x-route": "override" } }, "models": { "m": {} } } } }),
        json!({ "provider": { "local": { "npm": "@ai-sdk/openai-compatible", "options": { "baseURL": CANONICAL }, "models": { "m": { "provider": { "npm": "other-sdk" } } } } } }),
        json!({ "provider": { "local": { "npm": "@ai-sdk/openai-compatible", "options": { "baseURL": CANONICAL }, "models": { "m": { "provider": { "api": "https://model.example/v1" } } } } } }),
        json!({ "provider": { "local": { "npm": "@ai-sdk/openai-compatible", "options": { "baseURL": CANONICAL }, "models": { "m": { "options": { "baseURL": "https://model.example/v1" } } } } } }),
        json!({ "provider": { "local": { "npm": "@ai-sdk/openai-compatible", "options": { "baseURL": CANONICAL }, "models": { "m": { "headers": { "x-route": "override" } } } } } }),
        json!({ "provider": { "local": { "npm": "@ai-sdk/openai-compatible", "options": { "baseURL": CANONICAL }, "models": { "m": { "variants": { "custom": { "baseURL": "https://model.example/v1" } } } } } } }),
    ];

    for fixture in fixtures {
        let result =
            analyze(fixture).expect("unsupported route override remains a safe partial result");
        assert!(result.routes.is_empty());
        assert_eq!(result.coverage.excluded_count, 1);
    }
}

#[test]
fn nested_transport_overrides_and_excessive_depth_fail_closed() {
    let provider_nested_override = analyze(json!({
        "provider": {
            "local": {
                "npm": "@ai-sdk/openai-compatible",
                "options": {
                    "baseURL": CANONICAL,
                    "transport": { "request": { "fetch": "custom-fetch" } }
                },
                "models": { "wire-id": {} }
            }
        }
    }))
    .expect("a provider nested route override is a partial exclusion");
    assert!(provider_nested_override.routes.is_empty());
    assert_eq!(provider_nested_override.coverage.excluded_count, 1);

    let model_nested_override = analyze(json!({
        "provider": {
            "local": {
                "npm": "@ai-sdk/openai-compatible",
                "options": { "baseURL": CANONICAL },
                "models": { "wire-id": { "options": { "request": { "endpoint": "https://other.example" } } } }
            }
        }
    }))
    .expect("a model nested route override is a partial exclusion");
    assert!(model_nested_override.routes.is_empty());

    let nested_reasoning = analyze(json!({
        "provider": {
            "local": {
                "npm": "@ai-sdk/openai-compatible",
                "options": { "baseURL": CANONICAL, "reasoning": { "effort": "high" } },
                "models": { "wire-id": { "reasoning": { "budget": 123 } } }
            }
        }
    }))
    .expect("ordinary nested reasoning does not alter transport");
    assert_eq!(nested_reasoning.routes.len(), 1);

    let mut nested_reasoning = json!({ "reasoning": { "budget": 123 } });
    for _ in 0..9 {
        nested_reasoning = json!({ "reasoning": nested_reasoning });
    }
    let too_deep = json!({
        "provider": {
            "local": {
                "npm": "@ai-sdk/openai-compatible",
                "options": { "baseURL": CANONICAL, "settings": nested_reasoning },
                "models": { "wire-id": {} }
            }
        }
    });
    assert_eq!(
        analyze(too_deep).err().expect("bounded depth error"),
        OpenCodeRouteAnalysisError::LimitExceeded
    );
}

#[test]
fn known_wire_model_ids_must_be_nonempty_bounded_strings() {
    let malformed_type = json!({
        "provider": { "local": { "npm": "@ai-sdk/openai-compatible", "options": { "baseURL": CANONICAL }, "models": { "alias": { "id": 42 } } } }
    });
    let empty_id = json!({
        "provider": { "local": { "npm": "@ai-sdk/openai-compatible", "options": { "baseURL": CANONICAL }, "models": { "alias": { "id": "" } } } }
    });
    let long_id = json!({
        "provider": { "local": { "npm": "@ai-sdk/openai-compatible", "options": { "baseURL": CANONICAL }, "models": { "alias": { "id": "x" } } } }
    });
    let mut long_id = long_id;
    long_id["provider"]["local"]["models"]["alias"]["id"] = Value::String("x".repeat(513));

    for fixture in [malformed_type, empty_id, long_id] {
        assert!(
            analyze(fixture).is_err(),
            "malformed wire model IDs must not be classified"
        );
    }

    let arbitrary_valid_id = analyze(json!({
        "provider": { "local": { "npm": "@ai-sdk/openai-compatible", "options": { "baseURL": CANONICAL }, "models": { "friendly-alias": { "id": "vendor/wire-model-variant" } } } }
    }))
    .expect("valid arbitrary wire IDs remain supported");
    assert_eq!(arbitrary_valid_id.routes.len(), 1);
}

#[test]
fn malformed_schema_and_inventory_limits_fail_with_fixed_errors() {
    assert_eq!(
        analyze(json!({ "provider": [] }))
            .err()
            .expect("fixed error"),
        OpenCodeRouteAnalysisError::InvalidSchema
    );
    assert_eq!(
        analyze(json!({ "model": 42 })).err().expect("fixed error"),
        OpenCodeRouteAnalysisError::InvalidSchema
    );
    assert_eq!(
        analyze(json!({ "agent": { "build": { "model": false } } }))
            .err()
            .expect("fixed error"),
        OpenCodeRouteAnalysisError::InvalidSchema
    );

    let providers = (0..33)
        .map(|i| {
            (
                format!("p{i}"),
                custom_provider(CANONICAL, "@ai-sdk/openai-compatible"),
            )
        })
        .collect::<Map<_, _>>();
    assert_eq!(
        analyze(json!({ "provider": providers }))
            .err()
            .expect("fixed error"),
        OpenCodeRouteAnalysisError::LimitExceeded
    );

    let long_key = "p".repeat(129);
    let mut long_provider = Map::new();
    long_provider.insert(
        long_key,
        custom_provider(CANONICAL, "@ai-sdk/openai-compatible"),
    );
    assert_eq!(
        analyze(json!({ "provider": long_provider }))
            .err()
            .expect("fixed error"),
        OpenCodeRouteAnalysisError::LimitExceeded
    );
}

#[test]
fn stable_endpoint_must_be_the_owned_loopback_v1_address() {
    let root: Map<String, Value> = json!({ "model": "openai/model" })
        .as_object()
        .unwrap()
        .clone();
    assert_eq!(
        analyze_opencode_v1_routes(&root, "https://127.0.0.1:45123/v1")
            .err()
            .expect("fixed error"),
        OpenCodeRouteAnalysisError::InvalidEndpoint
    );
}
