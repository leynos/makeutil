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
    """Load the cached project base merged with local repository policy."""
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
    """Refresh the project base cache and write the merged configuration."""
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
