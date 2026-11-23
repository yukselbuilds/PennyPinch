"""Narrow, pinned workaround for Headroom 0.45.0's legacy cache marker."""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any, Callable


EXPECTED_VERSION = "0.45.0"
EXPECTED_PREFIX_TRACKER_SHA256 = "35015786e675bc435d92a6fed223b4282b03673eeb41b68cca50772dc69847dc"

# These are documented Anthropic Messages block types. Any other type is
# treated as opaque so the engine-generated legacy marker cannot mutate it.
KNOWN_CONTENT_BLOCK_TYPES = frozenset(
    {
        "text",
        "image",
        "document",
        "search_result",
        "tool_use",
        "tool_result",
        "thinking",
        "redacted_thinking",
        "server_tool_use",
        "web_search_tool_result",
        "web_search_result",
        "web_fetch_tool_result",
        "mcp_tool_use",
        "mcp_tool_result",
        "code_execution_tool_result",
        "bash_code_execution_tool_result",
        "text_editor_code_execution_tool_result",
        "computer_use_result",
        "tool_search_tool_result",
        "compaction",
        "container_upload",
    }
)


def _has_client_markers(messages: Any) -> bool:
    if not isinstance(messages, list):
        return False
    return any(
        isinstance(block, dict) and isinstance(block.get("cache_control"), dict)
        for message in messages
        if isinstance(message, dict) and isinstance(message.get("content"), list)
        for block in message["content"]
    )


def _client_opaque_markers(messages: Any) -> list[tuple[int, int, dict[str, Any], dict[str, Any]]]:
    markers: list[tuple[int, int, dict[str, Any], dict[str, Any]]] = []
    if not isinstance(messages, list):
        return markers
    for message_index, message in enumerate(messages):
        content = message.get("content") if isinstance(message, dict) else None
        if not isinstance(content, list):
            continue
        for block_index, block in enumerate(content):
            if (
                isinstance(block, dict)
                and block.get("type") not in KNOWN_CONTENT_BLOCK_TYPES
                and isinstance(block.get("cache_control"), dict)
            ):
                markers.append((message_index, block_index, block, block["cache_control"]))
    return markers


def _restore_client_opaque_markers(messages: Any, client_messages: Any) -> Any:
    targets = _client_opaque_markers(client_messages)
    if not targets:
        return messages
    if not isinstance(messages, list) or not isinstance(client_messages, list) or len(messages) != len(client_messages):
        raise RuntimeError("opaque-client-marker-message-alignment-failed")

    explicit_positions = {
        (message_index, block_index)
        for message_index, message in enumerate(client_messages)
        if isinstance(message, dict) and isinstance(message.get("content"), list)
        for block_index, block in enumerate(message["content"])
        if isinstance(block, dict) and isinstance(block.get("cache_control"), dict)
    }
    rewritten = list(messages)
    content_copies: dict[int, list[Any]] = {}
    for message_index, block_index, source_block, marker in targets:
        output_message = rewritten[message_index]
        output_content = output_message.get("content") if isinstance(output_message, dict) else None
        if not isinstance(output_content, list) or block_index >= len(output_content):
            raise RuntimeError("opaque-client-marker-block-alignment-failed")
        output_block = output_content[block_index]
        source_payload = {key: value for key, value in source_block.items() if key != "cache_control"}
        output_payload = (
            {key: value for key, value in output_block.items() if key != "cache_control"}
            if isinstance(output_block, dict)
            else None
        )
        if output_payload != source_payload:
            raise RuntimeError("opaque-client-marker-payload-changed")
        content_copies.setdefault(message_index, list(output_content))

        # Remove a relocated copy only when it is not itself at a current
        # client breakpoint. Then restore the original unknown block marker.
        for other_message_index, other_message in enumerate(rewritten):
            other_content = other_message.get("content") if isinstance(other_message, dict) else None
            if not isinstance(other_content, list):
                continue
            for other_block_index, other_block in enumerate(other_content):
                if (other_message_index, other_block_index) in explicit_positions:
                    continue
                if isinstance(other_block, dict) and other_block.get("cache_control") == marker:
                    copies = content_copies.setdefault(other_message_index, list(other_content))
                    clean_block = dict(other_block)
                    del clean_block["cache_control"]
                    copies[other_block_index] = clean_block

        copies = content_copies[message_index]
        restored = dict(source_block)
        restored["cache_control"] = dict(marker)
        copies[block_index] = restored

    for message_index, content in content_copies.items():
        message = rewritten[message_index]
        rewritten[message_index] = {**message, "content": content}
    return rewritten


def _protect_opaque_blocks(messages: Any) -> Any:
    if not isinstance(messages, list):
        return messages

    rewritten = list(messages)
    changed = False
    for message_index, message in enumerate(messages):
        content = message.get("content") if isinstance(message, dict) else None
        if not isinstance(content, list):
            continue
        new_content = list(content)
        message_changed = False
        for block_index, block in enumerate(content):
            if not isinstance(block, dict) or "cache_control" not in block:
                continue
            if block.get("type") in KNOWN_CONTENT_BLOCK_TYPES:
                continue

            marker = block.get("cache_control")
            if not isinstance(marker, dict):
                continue
            new_block = dict(block)
            del new_block["cache_control"]
            new_content[block_index] = new_block
            message_changed = True

            # Keep Headroom's generated cache breakpoint when there is a
            # preceding text block. Explicit client markers bypass this
            # adaptation entirely and retain their original positions/values.
            target = next(
                (
                    index
                    for index in range(block_index - 1, -1, -1)
                    if isinstance(new_content[index], dict)
                    and new_content[index].get("type") == "text"
                    and "cache_control" not in new_content[index]
                ),
                None,
            )
            if target is not None:
                target_block = dict(new_content[target])
                target_block["cache_control"] = dict(marker)
                new_content[target] = target_block

        if message_changed:
            rewritten[message_index] = {**message, "content": new_content}
            changed = True
    return rewritten if changed else messages


def install() -> dict[str, str]:
    """Patch only the exact pinned upstream implementation; otherwise fail closed."""
    import headroom._version
    import headroom.cache.prefix_tracker as prefix_tracker

    if headroom._version.__version__ != EXPECTED_VERSION:
        raise RuntimeError("headroom-version-mismatch")
    source_path = Path(prefix_tracker.__file__).resolve()
    source_sha256 = hashlib.sha256(source_path.read_bytes()).hexdigest()
    if source_sha256 != EXPECTED_PREFIX_TRACKER_SHA256:
        raise RuntimeError("prefix-tracker-source-mismatch")

    current: Callable[..., Any] = prefix_tracker.normalize_message_cache_control
    if getattr(current, "__headroom_proof_shim__", False):
        wrapped = current
    else:
        original = current

        def normalize_message_cache_control(
            messages: list[dict[str, Any]],
            previous_forwarded_messages: list[dict[str, Any]] | None = None,
            client_messages: list[dict[str, Any]] | None = None,
            *,
            place_unmarked: bool = True,
        ) -> list[dict[str, Any]]:
            normalized = original(
                messages, previous_forwarded_messages, client_messages, place_unmarked=place_unmarked
            )
            if _has_client_markers(client_messages):
                normalized = _restore_client_opaque_markers(normalized, client_messages)
                return normalized
            return _protect_opaque_blocks(normalized)

        normalize_message_cache_control.__headroom_proof_shim__ = True  # type: ignore[attr-defined]
        normalize_message_cache_control.__wrapped__ = original  # type: ignore[attr-defined]
        prefix_tracker.normalize_message_cache_control = normalize_message_cache_control
        wrapped = normalize_message_cache_control

    return {
        "version": headroom._version.__version__,
        "source_sha256": source_sha256,
        "shim_sha256": hashlib.sha256(Path(__file__).resolve().read_bytes()).hexdigest(),
        "active": "true" if getattr(wrapped, "__headroom_proof_shim__", False) else "false",
    }
