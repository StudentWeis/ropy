#!/usr/bin/env python3
"""Print a watch-friendly snapshot of active and historical coordination tasks."""

import argparse
import os
import sys
from pathlib import Path
from typing import Any

from coord import (
    CoordinationError,
    StateStore,
    find_repo,
    is_stale,
    resolve_state_dir,
    utc_now,
)

DEFAULT_HISTORY_LIMIT = 10
TITLE_LIMIT = 50
RESET = "\033[0m"
BOLD_GREEN = "\033[1;32m"
YELLOW = "\033[33m"
STATUS_COLORS = {
    "working": "\033[32m",
    "waiting": YELLOW,
    "blocked": "\033[31m",
    "stale": "\033[2;37m",
    "completed": "\033[32m",
    "abandoned": "\033[31m",
    "superseded": "\033[36m",
}


def clean_title(record: dict[str, Any]) -> str:
    title = " ".join(str(record.get("task", "")).split()) or "(untitled)"
    if len(title) <= TITLE_LIMIT:
        return title
    return f"{title[: TITLE_LIMIT - 1]}…"


def styled(value: str, color: str, enabled: bool) -> str:
    return f"{color}{value}{RESET}" if enabled else value


def render_status(store: StateStore, history_limit: int, color: bool) -> None:
    now = utc_now()
    with store.locked(exclusive=False):
        active, corrupt_active = store.load_active()
        history, corrupt_history = store.load_history()

    active.sort(key=lambda record: (record.get("started_at", ""), record["session_id"]))
    history.sort(
        key=lambda record: (record.get("finished_at", ""), record["session_id"]),
        reverse=True,
    )
    shown_history = history if history_limit == 0 else history[:history_limit]

    print(styled(f"ACTIVE ({len(active)})", BOLD_GREEN, color))
    if active:
        for record in active:
            status = record["status"]
            color_key = status
            if is_stale(record, now):
                status = f"{status}, stale"
                color_key = "stale"
            label = styled(status, STATUS_COLORS[color_key], color)
            print(f"- [{label}] {clean_title(record)}")
    else:
        print("- none")

    print()
    print(styled(f"HISTORY ({len(shown_history)}/{len(history)})", BOLD_GREEN, color))
    if shown_history:
        for record in shown_history:
            result = record["result"]
            label = styled(result, STATUS_COLORS[result], color)
            print(f"- [{label}] {clean_title(record)}")
    else:
        print("- none")

    corrupt = [("active", item) for item in corrupt_active] + [
        ("history", item) for item in corrupt_history
    ]
    if corrupt:
        print()
        print(styled(f"WARNINGS ({len(corrupt)})", YELLOW, color))
        for location, item in corrupt:
            print(f"- {location}/{item['file']}: {item['error']}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo", help="repository path; defaults to the current directory"
    )
    parser.add_argument(
        "--state-dir",
        help="coordination state directory; overrides AGENT_COORDINATION_DIR",
    )
    parser.add_argument(
        "--history-limit",
        type=int,
        default=DEFAULT_HISTORY_LIMIT,
        help="number of recent historical tasks to show; 0 shows all (default: 10)",
    )
    color_group = parser.add_mutually_exclusive_group()
    color_group.add_argument(
        "--color", action="store_true", help="force ANSI colors, including under watch"
    )
    color_group.add_argument(
        "--no-color", action="store_true", help="disable ANSI colors"
    )
    return parser


def main() -> int:
    args = build_parser().parse_args()
    try:
        if args.history_limit < 0:
            raise CoordinationError("--history-limit cannot be negative")
        repo = find_repo(Path(args.repo or Path.cwd()).resolve())
        store = StateStore(resolve_state_dir(repo, args.state_dir))
        color = args.color or (
            not args.no_color and "NO_COLOR" not in os.environ and sys.stdout.isatty()
        )
        render_status(store, args.history_limit, color)
        return 0
    except (CoordinationError, OSError) as error:
        print(f"coordination error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
