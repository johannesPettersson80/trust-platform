"""Injection-safe exact nextest name matchers."""

from __future__ import annotations


SAFE_MATCHER_CHARACTERS = frozenset(
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_.:-"
)


def exact_name_matcher(value: str) -> str:
    """Encode an exact matcher without exposing filterset syntax characters."""

    if not value:
        raise ValueError("nextest exact matcher requires a non-empty value")
    encoded: list[str] = []
    for character in value:
        codepoint = ord(character)
        if 0xD800 <= codepoint <= 0xDFFF:
            raise ValueError("nextest exact matcher cannot encode a surrogate")
        if character in SAFE_MATCHER_CHARACTERS:
            encoded.append(character)
        else:
            encoded.append(f"\\u{{{codepoint:X}}}")
    return "=" + "".join(encoded)
