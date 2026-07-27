#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = []
# ///
"""Generate ``typos.toml`` from the project-owned Oxford English dictionary.

The dictionary is refreshed into an untracked repository-local cache only when
the authoritative copy is newer. ``typos.local.toml`` supplies the narrow
repository-specific policy that must not weaken the project-owned base.
"""

from pathlib import Path

import typos_rollout as rollout

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_BASE_SOURCE = REPOSITORY_ROOT / "data" / "typos-oxendict-base.toml"


def dictionary_from_cache(repository: Path = REPOSITORY_ROOT) -> rollout.Dictionary:
    """Load the cached project base merged with local repository policy.

    Parameters
    ----------
    repository
        Repository containing the cached base and optional local overlay.

    Returns
    -------
    rollout.Dictionary
        The validated base dictionary merged with local policy.

    Raises
    ------
    OSError
        If a dictionary file cannot be read.
    TypeError
        If a dictionary contains values of the wrong type.
    ValueError
        If a dictionary is invalid or overlay corrections conflict.
    """
    dictionary = rollout.load_dictionary(repository / ".typos-oxendict-base.toml")
    local_overlay = repository / "typos.local.toml"
    if local_overlay.exists():
        dictionary = rollout.merge_dictionaries(
            dictionary,
            rollout.load_dictionary(local_overlay),
        )
    return dictionary


def render_config(repository: Path = REPOSITORY_ROOT) -> str:
    """Render deterministic configuration from the populated local cache."""
    return rollout.render_typos_config(dictionary_from_cache(repository))


def main(
    output: Path | None = None,
    *,
    repository: Path = REPOSITORY_ROOT,
    source: str | Path = DEFAULT_BASE_SOURCE,
    offline: bool = False,
) -> rollout.RefreshResult:
    """Refresh the base cache and write the merged repository configuration.

    Parameters
    ----------
    output
        Destination configuration, or ``repository / "typos.toml"`` when
        omitted.
    repository
        Repository containing cache metadata and local dictionary policy.
    source
        Local path or URL for the authoritative base dictionary.
    offline
        Use the existing valid cache without contacting or reading ``source``.

    Returns
    -------
    rollout.RefreshResult
        Cache status and the cache path used to render the configuration.

    Raises
    ------
    FileNotFoundError
        If offline mode is requested without a valid cached dictionary.
    OSError
        If source, cache, metadata, overlay, or output access fails.
    TypeError
        If dictionary data contains values of the wrong type.
    ValueError
        If dictionary data is invalid or overlay corrections conflict.
    """
    result = rollout.refresh_base(
        source,
        repository / ".typos-oxendict-base.toml",
        metadata=repository / ".typos-oxendict-base.json",
        offline=offline,
    )
    destination = output if output is not None else repository / "typos.toml"
    rollout.write_config(destination, dictionary_from_cache(repository))
    return result


if __name__ == "__main__":
    refresh = main()
    print(f"{refresh.status}: {REPOSITORY_ROOT / 'typos.toml'}")
