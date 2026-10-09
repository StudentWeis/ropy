#!/usr/bin/env python3
"""Advisory coordination ledger for concurrent repository agents."""

import argparse
import fcntl
import json
import os
import re
import secrets
import subprocess
import sys
import tempfile
import time
from collections.abc import Iterator
from contextlib import contextmanager
from datetime import UTC, datetime, timedelta
from pathlib import Path, PurePosixPath
from typing import Any, BinaryIO

SCHEMA_VERSION = 1
DEFAULT_LEASE_MINUTES = 120
SESSION_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
ACTIVE_STATUSES = {"working", "waiting", "blocked"}
LEASE_HOLDING_STATUSES = {"working", "waiting"}
FINISHED_RESULTS = {"completed", "abandoned", "superseded"}


class CoordinationError(Exception):
    """A user-facing coordination error."""


def utc_now() -> datetime:
    return datetime.now(UTC)


def format_time(value: datetime) -> str:
    return value.astimezone(UTC).isoformat(timespec="seconds").replace("+00:00", "Z")


def parse_time(value: Any) -> datetime:
    if not isinstance(value, str):
        raise CoordinationError("timestamp is not a string")
    try:
        parsed = datetime.fromisoformat(value)
    except ValueError as error:
        raise CoordinationError(f"invalid timestamp: {value}") from error
    if parsed.tzinfo is None:
        raise CoordinationError(f"timestamp lacks a timezone: {value}")
    return parsed.astimezone(UTC)


def git_output(repo: Path, *arguments: str) -> str:
    completed = subprocess.run(
        ["git", "-C", str(repo), *arguments],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    if completed.returncode != 0:
        return ""
    return completed.stdout.strip()


def find_repo(start: Path) -> Path:
    root = git_output(start, "rev-parse", "--show-toplevel")
    if not root:
        raise CoordinationError(f"not inside a Git repository: {start}")
    return Path(root).resolve()


def resolve_state_dir(repo: Path, explicit: str | None) -> Path:
    configured = explicit or os.environ.get("AGENT_COORDINATION_DIR")
    if configured:
        path = Path(configured).expanduser()
        if not path.is_absolute():
            path = repo / path
        return path.resolve()
    return repo / ".agent-coordination"


def normalize_scope(raw_scope: str) -> str:
    candidate = raw_scope.strip()
    if not candidate:
        raise CoordinationError("scope cannot be empty")
    if any(character in candidate for character in "*?[]"):
        raise CoordinationError(f"scope must be a path prefix, not a glob: {raw_scope}")
    path = PurePosixPath(candidate)
    if path.is_absolute() or ".." in path.parts:
        raise CoordinationError(f"scope must stay within the repository: {raw_scope}")
    parts = [part for part in path.parts if part not in {"", "."}]
    return "." if not parts else "/".join(parts)


def normalize_scopes(raw_scopes: list[str]) -> list[str]:
    normalized = sorted({normalize_scope(scope) for scope in raw_scopes})
    minimal: list[str] = []
    for scope in normalized:
        if any(scope_contains(existing, scope) for existing in minimal):
            continue
        minimal.append(scope)
    return minimal


def scope_contains(parent: str, child: str) -> bool:
    if parent == ".":
        return True
    parent_parts = PurePosixPath(parent).parts
    child_parts = PurePosixPath(child).parts
    return (
        len(parent_parts) <= len(child_parts)
        and child_parts[: len(parent_parts)] == parent_parts
    )


def scopes_overlap(left: list[str], right: list[str]) -> list[dict[str, str]]:
    overlaps: list[dict[str, str]] = []
    for left_scope in left:
        for right_scope in right:
            if scope_contains(left_scope, right_scope) or scope_contains(
                right_scope, left_scope
            ):
                overlaps.append({"ours": left_scope, "theirs": right_scope})
    return overlaps


def validate_session_id(session_id: str) -> str:
    if not SESSION_ID_RE.fullmatch(session_id):
        raise CoordinationError(
            "session id must contain only letters, digits, dot, underscore, or hyphen"
        )
    return session_id


def generate_session_id(now: datetime) -> str:
    prefix = now.astimezone(UTC).strftime("%Y%m%dT%H%M%S")
    return f"{prefix}-{secrets.token_hex(4)}"


def record_scopes(record: dict[str, Any]) -> list[str]:
    key = "write_scopes" if record.get("mode") == "write" else "read_scopes"
    scopes = record.get(key)
    if not isinstance(scopes, list) or not all(
        isinstance(item, str) for item in scopes
    ):
        raise CoordinationError(f"record has invalid {key}")
    return scopes


def held_scopes(record: dict[str, Any]) -> list[str]:
    """兼容旧记录；blocked 只保护已经获准的路径，不占用等待中的路径。"""
    if "held_scopes" in record:
        return record["held_scopes"]
    return record_scopes(record) if record["status"] in LEASE_HOLDING_STATUSES else []


def refresh_held_scopes(
    record: dict[str, Any], previous: list[str], conflicts: list[dict[str, Any]]
) -> None:
    requested = record_scopes(record)
    # 扩大到父目录受阻时，仍保留原先的子目录；缩小时立即释放移除的部分。
    retained = [
        child
        for old in previous
        for new in requested
        for parent, child in [(old, new), (new, old)]
        if scope_contains(parent, child)
    ]
    blocking = [c for c in conflicts if c["severity"] == "blocking"]
    if any(c["reason"] == "corrupt-active-record" for c in blocking):
        record["held_scopes"] = normalize_scopes(retained)
        return
    occupied = [o["theirs"] for c in blocking for o in c["overlaps"]]
    record["held_scopes"] = normalize_scopes(
        [
            scope
            for scope in requested + retained
            if not scopes_overlap([scope], occupied)
        ]
    )


def validate_record(
    record: Any, expected_id: str, *, allowed_statuses: set[str] = ACTIVE_STATUSES
) -> dict[str, Any]:
    if not isinstance(record, dict):
        raise CoordinationError("record root is not an object")
    if record.get("schema_version") != SCHEMA_VERSION:
        raise CoordinationError("unsupported schema_version")
    if record.get("session_id") != expected_id:
        raise CoordinationError("session_id does not match filename")
    validate_session_id(expected_id)
    if record.get("mode") not in {"read", "write"}:
        raise CoordinationError("record has invalid mode")
    if record.get("status") not in allowed_statuses:
        raise CoordinationError("record has invalid status")
    scopes = record_scopes(record)
    normalized = normalize_scopes(scopes)
    if normalized != scopes:
        raise CoordinationError("record scopes are not normalized")
    if "held_scopes" in record:
        held = record["held_scopes"]
        if (
            not isinstance(held, list)
            or not all(isinstance(scope, str) for scope in held)
            or normalize_scopes(held) != held
            or any(
                not any(scope_contains(scope, item) for scope in normalized)
                for item in held
            )
        ):
            raise CoordinationError("record has invalid held_scopes")
    parse_time(record.get("heartbeat_at"))
    parse_time(record.get("lease_expires_at"))
    return record


def validate_history_record(record: Any, expected_id: str) -> dict[str, Any]:
    validated = validate_record(record, expected_id, allowed_statuses={"finished"})
    if validated.get("result") not in FINISHED_RESULTS:
        raise CoordinationError("history record has invalid result")
    parse_time(validated.get("finished_at"))
    return validated


def is_stale(record: dict[str, Any], now: datetime) -> bool:
    return now >= parse_time(record["lease_expires_at"])


def renew_lease(record: dict[str, Any], now: datetime) -> None:
    lease_minutes = record.get("lease_minutes", DEFAULT_LEASE_MINUTES)
    if not isinstance(lease_minutes, int) or lease_minutes < 1:
        raise CoordinationError("record has invalid lease_minutes")
    record["heartbeat_at"] = format_time(now)
    record["lease_expires_at"] = format_time(now + timedelta(minutes=lease_minutes))
    record["updated_at"] = format_time(now)


def acquire_lock(handle: BinaryIO, *, exclusive: bool) -> None:
    operation = fcntl.LOCK_EX if exclusive else fcntl.LOCK_SH
    fcntl.flock(handle.fileno(), operation)


def release_lock(handle: BinaryIO) -> None:
    fcntl.flock(handle.fileno(), fcntl.LOCK_UN)


class StateStore:
    def __init__(self, root: Path):
        self.root = root
        self.active = root / "active"
        self.history = root / "history"
        self.lock_file = root / "coordination.lock"
        self.active.mkdir(parents=True, exist_ok=True)
        self.history.mkdir(parents=True, exist_ok=True)
        gitignore = root / ".gitignore"
        if not gitignore.exists():
            gitignore.write_text("*\n", encoding="utf-8")

    @contextmanager
    def locked(self, *, exclusive: bool, cleanup: bool = True) -> Iterator[None]:
        with self.lock_file.open("a+b") as lock_handle:
            acquire_lock(lock_handle, exclusive=exclusive or cleanup)
            try:
                if cleanup:
                    self.cleanup_expired(utc_now())
                yield
            finally:
                release_lock(lock_handle)

    def cleanup_expired(self, now: datetime) -> dict[str, Any]:
        """Archive expired sessions while holding the exclusive ledger lock."""
        archived: list[dict[str, str]] = []
        records, corrupt = self.load_active()
        for record in records:
            if not is_stale(record, now):
                continue
            record["status"] = "finished"
            record["result"] = "abandoned"
            record["summary"] = "Expired lease archived by cleanup."
            record["finished_at"] = format_time(now)
            record["updated_at"] = format_time(now)
            record["blocked_by"] = []
            destination = self.archive(record, now, expired_cleanup=True)
            archived.append(
                {"session_id": record["session_id"], "archived_to": str(destination)}
            )
        return {"archived": archived, "corrupt": corrupt, "state_dir": str(self.root)}

    def load_active(
        self, *, exclude_id: str | None = None
    ) -> tuple[list[dict[str, Any]], list[dict[str, str]]]:
        records: list[dict[str, Any]] = []
        corrupt: list[dict[str, str]] = []
        for path in sorted(self.active.glob("*.json")):
            if path.stem == exclude_id:
                continue
            try:
                with path.open(encoding="utf-8") as handle:
                    record = validate_record(json.load(handle), path.stem)
            except (CoordinationError, json.JSONDecodeError, OSError) as error:
                corrupt.append({"file": path.name, "error": str(error)})
                continue
            records.append(record)
        return records, corrupt

    def load_one(self, session_id: str) -> dict[str, Any]:
        validated_id = validate_session_id(session_id)
        path = self.active / f"{validated_id}.json"
        if not path.exists():
            raise CoordinationError(f"active session not found: {validated_id}")
        try:
            with path.open(encoding="utf-8") as handle:
                return validate_record(json.load(handle), validated_id)
        except (json.JSONDecodeError, OSError) as error:
            raise CoordinationError(
                f"cannot read active session {validated_id}: {error}"
            ) from error

    def load_history(self) -> tuple[list[dict[str, Any]], list[dict[str, str]]]:
        records: list[dict[str, Any]] = []
        corrupt: list[dict[str, str]] = []
        for path in sorted(self.history.glob("*/*.json")):
            try:
                with path.open(encoding="utf-8") as handle:
                    record = validate_history_record(json.load(handle), path.stem)
            except (CoordinationError, json.JSONDecodeError, OSError) as error:
                corrupt.append(
                    {"file": str(path.relative_to(self.history)), "error": str(error)}
                )
                continue
            records.append(record)
        return records, corrupt

    def save_active(self, record: dict[str, Any]) -> Path:
        path = self.active / f"{validate_session_id(record['session_id'])}.json"
        atomic_write_json(path, record)
        return path

    def archive(
        self, record: dict[str, Any], now: datetime, *, expired_cleanup: bool = False
    ) -> Path:
        session_id = validate_session_id(record["session_id"])
        existing = list(self.history.glob(f"*/{session_id}.json"))
        if existing:
            if len(existing) != 1:
                raise CoordinationError(f"multiple history records exist: {session_id}")
            destination = existing[0]
            with destination.open(encoding="utf-8") as handle:
                archived = validate_history_record(json.load(handle), session_id)
            # 重试会重新生成完成时间；其余字段必须一致，避免清理已变更的会话。
            retry_fields = {"finished_at", "updated_at"}
            if expired_cleanup:
                # 完成历史已落盘、活跃记录删除中断时，保留原完成结果；会话其余字段仍须匹配。
                retry_fields |= {"status", "result", "summary", "blocked_by"}
            if {k: v for k, v in archived.items() if k not in retry_fields} != {
                k: v for k, v in record.items() if k not in retry_fields
            }:
                raise CoordinationError(f"history record conflicts: {destination}")
            (self.active / f"{session_id}.json").unlink()
            record.update(archived)
            return destination
        day_directory = self.history / now.astimezone(UTC).strftime("%Y-%m-%d")
        day_directory.mkdir(parents=True, exist_ok=True)
        destination = (
            day_directory / f"{validate_session_id(record['session_id'])}.json"
        )
        if destination.exists():
            raise CoordinationError(f"history record already exists: {destination}")
        atomic_write_json(destination, record)
        active_path = self.active / f"{record['session_id']}.json"
        active_path.unlink()
        return destination


def atomic_write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        dir=path.parent, prefix=f".{path.name}.", suffix=".tmp"
    )
    temporary_path = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary_path, path)
    finally:
        temporary_path.unlink(missing_ok=True)


def atomic_write_json(path: Path, value: dict[str, Any]) -> None:
    content = json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    atomic_write_text(path, content)


def find_conflicts(
    candidate: dict[str, Any],
    records: list[dict[str, Any]],
    corrupt: list[dict[str, str]],
    now: datetime,
) -> list[dict[str, Any]]:
    conflicts: list[dict[str, Any]] = []
    candidate_mode = candidate["mode"]
    candidate_scopes = record_scopes(candidate)

    for bad_record in corrupt:
        conflicts.append(
            {
                "severity": "blocking" if candidate_mode == "write" else "warning",
                "reason": "corrupt-active-record",
                **bad_record,
            }
        )

    for other in records:
        other_mode = other["mode"]
        overlaps = scopes_overlap(candidate_scopes, held_scopes(other))
        if not overlaps:
            continue
        stale = is_stale(other, now)
        if candidate_mode == "write" and other_mode == "write":
            severity = "stale" if stale else "blocking"
            reason = "write-write-overlap"
        else:
            severity = "stale" if stale else "warning"
            reason = "read-write-overlap"
        conflicts.append(
            {
                "severity": severity,
                "reason": reason,
                "session_id": other["session_id"],
                "task": other.get("task", ""),
                "status": other["status"],
                "mode": other_mode,
                "overlaps": overlaps,
            }
        )
    return conflicts


def blocking_ids(conflicts: list[dict[str, Any]]) -> list[str]:
    return sorted(
        {
            conflict["session_id"]
            for conflict in conflicts
            if conflict.get("severity") == "blocking" and "session_id" in conflict
        }
    )


def has_blocking_conflict(conflicts: list[dict[str, Any]]) -> bool:
    return any(conflict.get("severity") == "blocking" for conflict in conflicts)


def start_command(
    args: argparse.Namespace, repo: Path, store: StateStore
) -> dict[str, Any]:
    now = utc_now()
    scopes = normalize_scopes(args.scope or ([] if args.mode == "write" else ["."]))
    if args.mode == "write" and not scopes:
        raise CoordinationError("write sessions require at least one --scope")
    if not 1 <= args.lease_minutes <= 1440:
        raise CoordinationError("--lease-minutes must be between 1 and 1440")
    session_id = (
        validate_session_id(args.session_id)
        if args.session_id
        else generate_session_id(now)
    )
    branch = git_output(repo, "branch", "--show-current") or None
    record: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "session_id": session_id,
        "agent": args.agent,
        "task": args.task.strip(),
        "mode": args.mode,
        "status": "working",
        "phase": args.phase,
        "write_scopes": scopes if args.mode == "write" else [],
        "read_scopes": scopes if args.mode == "read" else [],
        "worktree": str(repo),
        "branch": branch,
        "started_at": format_time(now),
        "updated_at": format_time(now),
        "heartbeat_at": format_time(now),
        "lease_minutes": args.lease_minutes,
        "lease_expires_at": format_time(now + timedelta(minutes=args.lease_minutes)),
        "note": args.note or "",
        "blocked_by": [],
        "held_scopes": [],
    }
    if not record["task"]:
        raise CoordinationError("task cannot be empty")

    with store.locked(exclusive=True):
        if (store.active / f"{session_id}.json").exists():
            raise CoordinationError(f"active session already exists: {session_id}")
        if next(store.history.glob(f"*/{session_id}.json"), None) is not None:
            raise CoordinationError(f"session already archived: {session_id}")
        records, corrupt = store.load_active()
        conflicts = find_conflicts(record, records, corrupt, now)
        refresh_held_scopes(record, [], conflicts)
        if has_blocking_conflict(conflicts):
            record["status"] = "blocked"
            record["phase"] = "waiting"
            record["blocked_by"] = blocking_ids(conflicts)
        store.save_active(record)
    return {"record": record, "conflicts": conflicts, "state_dir": str(store.root)}


def update_command(args: argparse.Namespace, store: StateStore) -> dict[str, Any]:
    now = utc_now()
    with store.locked(exclusive=True):
        record = store.load_one(args.id)
        previous = held_scopes(record)
        if args.mode and args.mode != record["mode"] and args.scope is None:
            raise CoordinationError("changing mode requires at least one --scope")
        if args.mode:
            record["mode"] = args.mode
        if args.scope is not None:
            scopes = normalize_scopes(args.scope)
            if record["mode"] == "write" and not scopes:
                raise CoordinationError("write sessions require at least one --scope")
            record["write_scopes"] = scopes if record["mode"] == "write" else []
            record["read_scopes"] = scopes if record["mode"] == "read" else []
        if args.task is not None:
            task = args.task.strip()
            if not task:
                raise CoordinationError("task cannot be empty")
            record["task"] = task
        if args.phase is not None:
            record["phase"] = args.phase
        if args.note is not None:
            record["note"] = args.note

        desired_status = args.status or record["status"]
        record["status"] = desired_status
        renew_lease(record, now)
        records, corrupt = store.load_active(exclude_id=record["session_id"])
        conflicts = find_conflicts(record, records, corrupt, now)
        refresh_held_scopes(record, previous, conflicts)
        if desired_status in LEASE_HOLDING_STATUSES and has_blocking_conflict(
            conflicts
        ):
            record["status"] = "blocked"
            record["phase"] = "waiting"
        record["blocked_by"] = blocking_ids(conflicts)
        store.save_active(record)
    return {"record": record, "conflicts": conflicts, "state_dir": str(store.root)}


def heartbeat_command(args: argparse.Namespace, store: StateStore) -> dict[str, Any]:
    now = utc_now()
    with store.locked(exclusive=True):
        record = store.load_one(args.id)
        previous = held_scopes(record)
        if args.note is not None:
            record["note"] = args.note
        renew_lease(record, now)
        records, corrupt = store.load_active(exclude_id=record["session_id"])
        conflicts = find_conflicts(record, records, corrupt, now)
        refresh_held_scopes(record, previous, conflicts)
        if has_blocking_conflict(conflicts):
            record["status"] = "blocked"
            record["phase"] = "waiting"
        record["blocked_by"] = blocking_ids(conflicts)
        store.save_active(record)
    return {"record": record, "conflicts": conflicts, "state_dir": str(store.root)}


def wait_command(args: argparse.Namespace, store: StateStore) -> dict[str, Any]:
    deadline = utc_now() + timedelta(minutes=args.timeout_minutes)
    while True:
        with store.locked(exclusive=True):
            record = store.load_one(args.id)
            previous = held_scopes(record)
            now = utc_now()
            renew_lease(record, now)
            records, corrupt = store.load_active(exclude_id=record["session_id"])
            conflicts = find_conflicts(record, records, corrupt, now)
            refresh_held_scopes(record, previous, conflicts)
            blockers = blocking_ids(conflicts)
            blocked = has_blocking_conflict(conflicts)
            record["blocked_by"] = blockers
            if blocked:
                record["status"] = "blocked"
            elif record["status"] == "blocked":
                record["status"] = "working"
                record["updated_at"] = format_time(now)
            store.save_active(record)
            result = {
                "record": record,
                "conflicts": conflicts,
                "state_dir": str(store.root),
            }
        if not blocked:
            return result
        if utc_now() >= deadline:
            reasons = blockers + [item["file"] for item in corrupt]
            raise CoordinationError(
                f"still blocked by {', '.join(reasons)} after {args.timeout_minutes} minutes"
            )
        time.sleep(min(args.poll_seconds, 60))


def finish_command(args: argparse.Namespace, store: StateStore) -> dict[str, Any]:
    now = utc_now()
    with store.locked(exclusive=True):
        record = store.load_one(args.id)
        record["status"] = "finished"
        record["result"] = args.result
        record["summary"] = args.summary or ""
        record["finished_at"] = format_time(now)
        record["updated_at"] = format_time(now)
        record["blocked_by"] = []
        destination = store.archive(record, now)
    return {
        "record": record,
        "archived_to": str(destination),
        "state_dir": str(store.root),
    }


def cleanup_command(store: StateStore) -> dict[str, Any]:
    with store.locked(exclusive=True, cleanup=False):
        return store.cleanup_expired(utc_now())


def list_command(store: StateStore) -> dict[str, Any]:
    now = utc_now()
    with store.locked(exclusive=False):
        records, corrupt = store.load_active()
    decorated: list[dict[str, Any]] = []
    for record in records:
        item = dict(record)
        item["freshness"] = "stale" if is_stale(record, now) else "fresh"
        item["heartbeat_age_seconds"] = max(
            0, int((now - parse_time(record["heartbeat_at"])).total_seconds())
        )
        decorated.append(item)
    decorated.sort(key=lambda item: item.get("started_at", ""))
    return {"sessions": decorated, "corrupt": corrupt, "state_dir": str(store.root)}


def print_result(result: dict[str, Any], *, as_json: bool) -> None:
    if as_json:
        print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
        return
    record = result.get("record")
    if isinstance(record, dict):
        print(f"session_id={record['session_id']}")
        print(f"status={record['status']}")
        print(f"mode={record['mode']}")
        scopes = record_scopes(record)
        print(f"scopes={','.join(scopes)}")
        print(f"held_scopes={','.join(held_scopes(record))}")
        if "archived_to" in result:
            print(f"archived_to={result['archived_to']}")
        conflicts = result.get("conflicts", [])
        if conflicts:
            print("conflicts:")
            for conflict in conflicts:
                peer = conflict.get("session_id", conflict.get("file", "unknown"))
                print(
                    f"- {conflict.get('severity', 'warning')}: "
                    f"{conflict.get('reason', 'unknown')} ({peer})"
                )
        return

    if "archived" in result:
        print(f"Archived {len(result['archived'])} expired coordination sessions.")
        for item in result["archived"]:
            print(f"- {item['session_id']}: {item['archived_to']}")
        for corrupt in result.get("corrupt", []):
            print(f"CORRUPT\t{corrupt['file']}\t{corrupt['error']}")
        return

    sessions = result.get("sessions", [])
    if not sessions:
        print("No active coordination sessions.")
    else:
        print("SESSION\tSTATUS\tMODE\tFRESHNESS\tSCOPES\tTASK")
        for session in sessions:
            scopes = ",".join(record_scopes(session))
            task = str(session.get("task", "")).replace("\t", " ").replace("\n", " ")
            print(
                f"{session['session_id']}\t{session['status']}\t{session['mode']}\t"
                f"{session['freshness']}\t{scopes}\t{task}"
            )
    for corrupt in result.get("corrupt", []):
        print(f"CORRUPT\t{corrupt['file']}\t{corrupt['error']}")


def add_json_option(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--json", action="store_true", help="emit machine-readable JSON"
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo", help="repository path; defaults to the current directory"
    )
    parser.add_argument(
        "--state-dir",
        help="coordination state directory; overrides AGENT_COORDINATION_DIR",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    list_parser = subparsers.add_parser("list", help="list active sessions")
    add_json_option(list_parser)

    cleanup_parser = subparsers.add_parser(
        "cleanup", help="archive all expired sessions as abandoned"
    )
    add_json_option(cleanup_parser)

    start_parser = subparsers.add_parser("start", help="register a new session")
    start_parser.add_argument("--mode", choices=("read", "write"), default="write")
    start_parser.add_argument("--task", required=True)
    start_parser.add_argument("--scope", action="append")
    start_parser.add_argument("--phase", default="planning")
    start_parser.add_argument("--note")
    start_parser.add_argument("--agent", default=os.environ.get("AGENT_NAME", "codex"))
    start_parser.add_argument("--session-id")
    start_parser.add_argument(
        "--lease-minutes", type=int, default=DEFAULT_LEASE_MINUTES
    )
    add_json_option(start_parser)

    update_parser = subparsers.add_parser(
        "update", help="update task, scope, or status"
    )
    update_parser.add_argument("--id", required=True)
    update_parser.add_argument("--mode", choices=("read", "write"))
    update_parser.add_argument("--task")
    update_parser.add_argument("--scope", action="append")
    update_parser.add_argument("--phase")
    update_parser.add_argument("--note")
    update_parser.add_argument("--status", choices=tuple(sorted(ACTIVE_STATUSES)))
    add_json_option(update_parser)

    heartbeat_parser = subparsers.add_parser("heartbeat", help="renew a session lease")
    heartbeat_parser.add_argument("--id", required=True)
    heartbeat_parser.add_argument("--note")
    add_json_option(heartbeat_parser)

    wait_parser = subparsers.add_parser(
        "wait",
        help="block until a session's blocking conflicts clear, then mark it working",
    )
    wait_parser.add_argument("--id", required=True)
    wait_parser.add_argument(
        "--timeout-minutes",
        type=float,
        default=1440.0,
        help="give up after this many minutes instead of waiting forever",
    )
    wait_parser.add_argument(
        "--poll-seconds",
        type=float,
        default=15.0,
        help="seconds between conflict re-checks",
    )
    add_json_option(wait_parser)

    finish_parser = subparsers.add_parser("finish", help="archive a finished session")
    finish_parser.add_argument("--id", required=True)
    finish_parser.add_argument(
        "--result", choices=("completed", "abandoned", "superseded"), required=True
    )
    finish_parser.add_argument("--summary")
    add_json_option(finish_parser)

    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    try:
        repo = find_repo(Path(args.repo or Path.cwd()).resolve())
        state_dir = resolve_state_dir(repo, args.state_dir)
        store = StateStore(state_dir)
        if args.command == "start":
            result = start_command(args, repo, store)
        elif args.command == "update":
            result = update_command(args, store)
        elif args.command == "heartbeat":
            result = heartbeat_command(args, store)
        elif args.command == "wait":
            if args.timeout_minutes <= 0:
                raise CoordinationError("--timeout-minutes must be greater than zero")
            if args.poll_seconds <= 0:
                raise CoordinationError("--poll-seconds must be greater than zero")
            result = wait_command(args, store)
        elif args.command == "finish":
            result = finish_command(args, store)
        elif args.command == "cleanup":
            result = cleanup_command(store)
        else:
            result = list_command(store)
        print_result(result, as_json=args.json)
        return 0
    except (CoordinationError, OSError) as error:
        print(f"coordination error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
