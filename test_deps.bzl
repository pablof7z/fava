"""Shared helpers for BUILD-only test dependency closure."""

def deduplicated_deps(existing, extra):
    """Appends dependency labels that are not already present."""
    return existing + [dep for dep in extra if dep not in existing]
