"""Shared filtering for visible text and independently hashed Git evidence."""
import re


def redact(text: str) -> str:
    """Defense in depth for obvious credentials, not a claim to detect all secrets."""
    text = re.sub(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----", "[REDACTED PRIVATE KEY]", text)
    text = re.sub(r"(?i)\b(Bearer\s+)[A-Za-z0-9._~+/=-]{16,}", r"\1[REDACTED]", text)
    text = re.sub(r"\b(?:sk-[A-Za-z0-9_-]{20,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|xox[baprs]-[A-Za-z0-9-]{15,})", "[REDACTED TOKEN]", text)
    return text

