import importlib.util
import json
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from concurrent.futures import ThreadPoolExecutor
from datetime import UTC, datetime, timedelta
from pathlib import Path
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
SCRIPT = SCRIPTS / "coord.py"
STATUS_SCRIPT = SCRIPTS / "status.py"
SPEC = importlib.util.spec_from_file_location("coord", SCRIPT)
coord = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(coord)


class CoordinationCliTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.repo = Path(self.temporary_directory.name)
        subprocess.run(
            ["git", "init", "--quiet", str(self.repo)],
            check=True,
            capture_output=True,
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def run_coord(
        self, *arguments: str, check: bool = True
    ) -> subprocess.CompletedProcess[str]:
        completed = subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "--repo",
                str(self.repo),
                *arguments,
                "--json",
            ],
            check=False,
            capture_output=True,
            text=True,
        )
        if check and completed.returncode != 0:
            self.fail(f"coord command failed: {completed.stderr}")
        return completed

    def run_json(self, *arguments: str) -> dict[str, object]:
        return json.loads(self.run_coord(*arguments).stdout)

    def test_overlapping_writer_waits_until_the_first_finishes(self) -> None:
        first = self.run_json(
            "start", "--task", "first writer", "--scope", "crates/app"
        )
        second = self.run_json(
            "start", "--task", "second writer", "--scope", "crates/app/src/task.rs"
        )

        first_record = first["record"]
        second_record = second["record"]
        self.assertEqual(first_record["status"], "working")
        self.assertEqual(second_record["status"], "blocked")
        self.assertEqual(second_record["blocked_by"], [first_record["session_id"]])

        self.run_json(
            "finish",
            "--id",
            str(first_record["session_id"]),
            "--result",
            "completed",
        )
        resumed = self.run_json(
            "update",
            "--id",
            str(second_record["session_id"]),
            "--status",
            "working",
        )
        self.assertEqual(resumed["record"]["status"], "working")
        self.assertEqual(resumed["record"]["blocked_by"], [])

    def test_wait_resumes_automatically_once_the_blocker_finishes(self) -> None:
        first = self.run_json(
            "start", "--task", "first writer", "--scope", "crates/app"
        )
        second = self.run_json(
            "start", "--task", "second writer", "--scope", "crates/app/src/task.rs"
        )
        self.assertEqual(second["record"]["status"], "blocked")

        def finish_first() -> None:
            time.sleep(0.5)
            self.run_json(
                "finish",
                "--id",
                str(first["record"]["session_id"]),
                "--result",
                "completed",
            )

        thread = threading.Thread(target=finish_first, daemon=True)
        thread.start()
        try:
            resumed = self.run_json(
                "wait",
                "--id",
                str(second["record"]["session_id"]),
                "--timeout-minutes",
                "0.5",
                "--poll-seconds",
                "0.1",
            )
        finally:
            thread.join(timeout=5)
        self.assertEqual(resumed["record"]["status"], "working")
        self.assertEqual(resumed["record"]["blocked_by"], [])

    def test_wait_reports_timeout_while_still_blocked(self) -> None:
        self.run_json("start", "--task", "first writer", "--scope", "crates/app")
        second = self.run_json(
            "start", "--task", "second writer", "--scope", "crates/app/src/task.rs"
        )
        completed = self.run_coord(
            "wait",
            "--id",
            str(second["record"]["session_id"]),
            "--timeout-minutes",
            "0.01",
            "--poll-seconds",
            "0.1",
            check=False,
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("still blocked by", completed.stderr)

    def test_disjoint_writers_can_work_concurrently(self) -> None:
        first = self.run_json("start", "--task", "backend", "--scope", "crates")
        second = self.run_json("start", "--task", "frontend", "--scope", "apps/web")
        self.assertEqual(first["record"]["status"], "working")
        self.assertEqual(second["record"]["status"], "working")

    def test_wait_keeps_corrupt_records_blocking_until_removed(self) -> None:
        started = self.run_json("start", "--task", "writer", "--scope", "scripts")
        session_id = str(started["record"]["session_id"])
        corrupt = self.repo / ".agent-coordination/active/broken.json"
        corrupt.write_text("{")
        blocked = self.run_json("update", "--id", session_id)
        self.assertEqual(blocked["record"]["status"], "blocked")
        result = self.run_coord(
            "wait",
            "--id",
            session_id,
            "--timeout-minutes",
            "0.001",
            "--poll-seconds",
            "0.1",
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("broken.json", result.stderr)
        record = json.loads((corrupt.parent / f"{session_id}.json").read_text())
        self.assertEqual(record["status"], "blocked")
        corrupt.unlink()
        resumed = self.run_json("wait", "--id", session_id)
        self.assertEqual(resumed["record"]["status"], "working")

    def test_blocked_expansion_retains_previous_scope_until_wait_completes(
        self,
    ) -> None:
        first = self.run_json("start", "--task", "first", "--scope", "config/nvim")
        second = self.run_json("start", "--task", "second", "--scope", "config/zsh")
        first_id = str(first["record"]["session_id"])
        # 申请父目录也不能释放已经占用的子目录。
        expanded = self.run_json("update", "--id", first_id, "--scope", "config")
        self.assertEqual(expanded["record"]["status"], "blocked")
        self.assertEqual(expanded["record"]["held_scopes"], ["config/nvim"])
        third = self.run_json("start", "--task", "third", "--scope", "config/nvim")
        self.assertEqual(third["record"]["status"], "blocked")
        self.assertEqual(third["record"]["blocked_by"], [first_id])
        # 心跳不能把等待中的父目录当成已经占用，也不能丢掉原有占用。
        pulse = self.run_json("heartbeat", "--id", first_id)
        self.assertEqual(pulse["record"]["held_scopes"], ["config/nvim"])
        self.run_json(
            "finish",
            "--id",
            str(second["record"]["session_id"]),
            "--result",
            "completed",
        )
        resumed = self.run_json("wait", "--id", first_id, "--timeout-minutes", "0.01")
        self.assertEqual(resumed["record"]["status"], "working")
        self.assertEqual(resumed["record"]["held_scopes"], ["config"])

    def test_partial_claim_and_narrowing_release_only_removed_paths(self) -> None:
        self.run_json("start", "--task", "blocker", "--scope", "config/zsh")
        writer = self.run_json(
            "start", "--task", "writer", "--scope", "config/zsh", "--scope", "scripts"
        )
        writer_id = str(writer["record"]["session_id"])
        self.assertEqual(writer["record"]["held_scopes"], ["scripts"])
        narrowed = self.run_json(
            "update",
            "--id",
            writer_id,
            "--scope",
            "scripts/new.py",
            "--status",
            "working",
        )
        self.assertEqual(narrowed["record"]["held_scopes"], ["scripts/new.py"])
        free = self.run_json("start", "--task", "free", "--scope", "scripts/doctor.sh")
        busy = self.run_json("start", "--task", "busy", "--scope", "scripts/new.py")
        self.assertEqual(free["record"]["status"], "working")
        self.assertEqual(busy["record"]["status"], "blocked")

    def test_legacy_working_record_keeps_its_reservation(self) -> None:
        first = self.run_json("start", "--task", "legacy", "--scope", "scripts")
        record = first["record"]
        del record["held_scopes"]
        active = (
            self.repo
            / ".agent-coordination/active"
            / (str(record["session_id"]) + ".json")
        )
        active.write_text(json.dumps(record))
        second = self.run_json("start", "--task", "second", "--scope", "scripts")
        self.assertEqual(second["record"]["status"], "blocked")

    def test_reader_causes_warning_but_does_not_block_writer(self) -> None:
        self.run_json("start", "--mode", "read", "--task", "inspect repository")
        writer = self.run_json("start", "--task", "edit core", "--scope", "crates/core")
        self.assertEqual(writer["record"]["status"], "working")
        self.assertEqual(writer["conflicts"][0]["severity"], "warning")

    def test_simultaneous_claims_have_exactly_one_working_writer(self) -> None:
        def claim(number: int) -> dict[str, object]:
            return self.run_json(
                "start", "--task", f"writer {number}", "--scope", "shared/path"
            )

        with ThreadPoolExecutor(max_workers=6) as executor:
            results = list(executor.map(claim, range(6)))
        statuses = [result["record"]["status"] for result in results]
        self.assertEqual(statuses.count("working"), 1)
        self.assertEqual(statuses.count("blocked"), 5)

    def test_finish_moves_record_to_history(self) -> None:
        started = self.run_json(
            "start", "--task", "small change", "--scope", "README.md"
        )
        session_id = str(started["record"]["session_id"])
        finished = self.run_json(
            "finish",
            "--id",
            session_id,
            "--result",
            "completed",
            "--summary",
            "done",
        )
        archived_to = Path(str(finished["archived_to"]))
        self.assertTrue(archived_to.exists())
        active_path = self.repo / ".agent-coordination/active" / f"{session_id}.json"
        self.assertFalse(active_path.exists())

    def test_start_rejects_finished_session_id_without_blocking_the_ledger(
        self,
    ) -> None:
        session_id = "finished-session"
        self.run_json(
            "start", "--task", "first", "--scope", "shared", "--session-id", session_id
        )
        finished = self.run_json("finish", "--id", session_id, "--result", "completed")
        archived = Path(finished["archived_to"])
        original = archived.read_bytes()
        result = self.run_coord(
            "start",
            "--task",
            "second",
            "--scope",
            "shared",
            "--session-id",
            session_id,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("session already archived", result.stderr)
        self.assertEqual(archived.read_bytes(), original)
        self.assertEqual(self.run_json("list")["sessions"], [])
        replacement = self.run_json(
            "start", "--task", "replacement", "--scope", "shared"
        )
        self.assertEqual(replacement["record"]["status"], "working")
        self.run_json(
            "finish",
            "--id",
            replacement["record"]["session_id"],
            "--result",
            "completed",
        )
        self.assertEqual(self.run_json("list")["sessions"], [])

    def test_start_rejects_session_id_archived_by_automatic_expiry(self) -> None:
        session_id = "expired-session"
        self.run_json(
            "start",
            "--task",
            "expired",
            "--scope",
            "shared",
            "--session-id",
            session_id,
            "--lease-minutes",
            "1",
        )
        store = coord.StateStore(self.repo / ".agent-coordination")
        args = coord.build_parser().parse_args(
            [
                "start",
                "--task",
                "replacement",
                "--scope",
                "shared",
                "--session-id",
                session_id,
            ]
        )
        with (
            patch.object(
                coord,
                "utc_now",
                return_value=datetime.now(UTC) + timedelta(days=2),
            ),
            self.assertRaisesRegex(coord.CoordinationError, "session already archived"),
        ):
            coord.start_command(args, self.repo, store)
        self.assertEqual(self.run_json("list")["sessions"], [])
        history, corrupt = store.load_history()
        self.assertEqual(corrupt, [])
        self.assertEqual(history[0]["session_id"], session_id)
        self.assertEqual(history[0]["result"], "abandoned")
        replacement = self.run_json(
            "start", "--task", "replacement", "--scope", "shared"
        )
        self.assertEqual(replacement["record"]["status"], "working")

    def test_expired_session_is_archived_and_cannot_be_revived(self) -> None:
        store = coord.StateStore(self.repo / ".agent-coordination")
        record = self.run_json("start", "--task", "expired", "--scope", "shared")[
            "record"
        ]
        record["lease_expires_at"] = coord.format_time(
            datetime.now(UTC) - timedelta(seconds=1)
        )
        store.save_active(record)
        second = self.run_json("start", "--task", "replacement", "--scope", "shared")
        self.assertEqual(second["record"]["status"], "working")
        self.assertEqual(second["conflicts"], [])
        pulse = self.run_coord("heartbeat", "--id", record["session_id"], check=False)
        self.assertNotEqual(pulse.returncode, 0)
        self.assertIn("active session not found", pulse.stderr)
        history, corrupt = store.load_history()
        self.assertEqual(corrupt, [])
        self.assertEqual(history[0]["result"], "abandoned")

    def test_list_and_status_automatically_archive_expired_sessions(self) -> None:
        store = coord.StateStore(self.repo / ".agent-coordination")
        for command in ("list", "status"):
            with self.subTest(command=command):
                record = self.run_json("start", "--task", command, "--scope", command)[
                    "record"
                ]
                record["lease_expires_at"] = coord.format_time(
                    datetime.now(UTC) - timedelta(seconds=1)
                )
                store.save_active(record)
                if command == "list":
                    self.assertEqual(self.run_json("list")["sessions"], [])
                else:
                    result = subprocess.run(
                        [sys.executable, str(STATUS_SCRIPT), "--repo", str(self.repo)],
                        capture_output=True,
                        text=True,
                        check=True,
                    )
                    self.assertIn("ACTIVE (0)", result.stdout)
                    self.assertIn("[abandoned] status", result.stdout)
                self.assertFalse(
                    (store.active / f"{record['session_id']}.json").exists()
                )

    def test_interrupted_finish_retries_without_rewriting_history(self) -> None:
        for delay in (timedelta(seconds=5), timedelta(minutes=5)):
            with self.subTest(delay=delay):
                store = coord.StateStore(self.repo / ".agent-coordination")
                now = datetime(2026, 9, 18, 23, 59, tzinfo=UTC)
                with patch.object(coord, "utc_now", return_value=now):
                    started = coord.start_command(
                        coord.build_parser().parse_args(
                            ["start", "--task", "archive", "--scope", "shared"]
                        ),
                        self.repo,
                        store,
                    )
                session_id = started["record"]["session_id"]
                args = coord.build_parser().parse_args(
                    [
                        "finish",
                        "--id",
                        session_id,
                        "--result",
                        "completed",
                        "--summary",
                        "done",
                    ]
                )
                active_path = store.active / f"{session_id}.json"
                unlink = Path.unlink

                def interrupt(
                    path, *arguments, active_path=active_path, unlink=unlink, **keywords
                ):
                    if path == active_path:
                        raise OSError("simulated cleanup failure")
                    return unlink(path, *arguments, **keywords)

                with (
                    patch.object(coord, "utc_now", return_value=now),
                    patch.object(Path, "unlink", interrupt),
                    self.assertRaisesRegex(OSError, "simulated cleanup failure"),
                ):
                    coord.finish_command(args, store)
                history = list(store.history.glob(f"*/{session_id}.json"))
                self.assertEqual(len(history), 1)
                original = history[0].read_bytes()
                self.assertTrue(active_path.exists())
                with patch.object(coord, "utc_now", return_value=now + delay):
                    args.summary = "different completion"
                    with self.assertRaisesRegex(
                        coord.CoordinationError, "history record conflicts"
                    ):
                        coord.finish_command(args, store)
                    self.assertTrue(active_path.exists())
                    self.assertEqual(history[0].read_bytes(), original)
                    args.summary = "done"
                    finished = coord.finish_command(args, store)
                self.assertFalse(active_path.exists())
                self.assertEqual(
                    list(store.history.glob(f"*/{session_id}.json")), history
                )
                self.assertEqual(history[0].read_bytes(), original)
                self.assertEqual(finished["record"], json.loads(original))
                self.assertEqual(Path(finished["archived_to"]), history[0])

    def test_expired_cleanup_recovers_interrupted_finish_without_changing_history(
        self,
    ) -> None:
        for changed in (False, True):
            with (
                self.subTest(changed=changed),
                tempfile.TemporaryDirectory() as directory,
            ):
                store = coord.StateStore(Path(directory))
                parser = coord.build_parser()
                now = datetime.now(UTC)
                with patch.object(coord, "utc_now", return_value=now):
                    record = coord.start_command(
                        parser.parse_args(
                            [
                                "start",
                                "--task",
                                "archive",
                                "--scope",
                                "shared",
                                "--lease-minutes",
                                "1",
                            ]
                        ),
                        self.repo,
                        store,
                    )["record"]
                session_id = record["session_id"]
                active = store.active / f"{session_id}.json"
                unlink = Path.unlink

                def interrupt(
                    path, *arguments, active=active, unlink=unlink, **keywords
                ):
                    if path == active:
                        raise OSError("simulated cleanup failure")
                    return unlink(path, *arguments, **keywords)

                with (
                    patch.object(
                        coord, "utc_now", return_value=now + timedelta(seconds=5)
                    ),
                    patch.object(Path, "unlink", interrupt),
                    self.assertRaisesRegex(OSError, "simulated cleanup failure"),
                ):
                    coord.finish_command(
                        parser.parse_args(
                            [
                                "finish",
                                "--id",
                                session_id,
                                "--result",
                                "completed",
                                "--summary",
                                "done",
                            ]
                        ),
                        store,
                    )
                history = list(store.history.glob(f"*/{session_id}.json"))
                original = history[0].read_bytes()
                if changed:
                    record["task"] = "different task"
                    store.save_active(record)
                with patch.object(
                    coord, "utc_now", return_value=now + timedelta(days=1)
                ):
                    if changed:
                        with self.assertRaisesRegex(
                            coord.CoordinationError, "history record conflicts"
                        ):
                            coord.list_command(store)
                        self.assertTrue(active.exists())
                    else:
                        self.assertEqual(coord.list_command(store)["sessions"], [])
                        replacement = coord.start_command(
                            parser.parse_args(
                                ["start", "--task", "replacement", "--scope", "shared"]
                            ),
                            self.repo,
                            store,
                        )
                        self.assertEqual(replacement["record"]["status"], "working")
                        self.assertFalse(active.exists())
                self.assertEqual(
                    list(store.history.glob(f"*/{session_id}.json")), history
                )
                self.assertEqual(history[0].read_bytes(), original)

    def test_cleanup_archives_only_expired_sessions_and_preserves_corrupt(self) -> None:
        store = coord.StateStore(self.repo / ".agent-coordination")
        now = datetime.now(UTC)
        expired_ids = set()
        expired_records = []
        for status in ("working", "waiting", "blocked"):
            record = self.run_json("start", "--task", status, "--scope", status)[
                "record"
            ]
            record["status"] = status
            record["lease_expires_at"] = coord.format_time(now - timedelta(seconds=1))
            expired_records.append(record)
            expired_ids.add(record["session_id"])
        fresh = self.run_json("start", "--task", "fresh", "--scope", "fresh")["record"]
        fresh_path = store.active / f"{fresh['session_id']}.json"
        fresh_bytes = fresh_path.read_bytes()
        corrupt_path = store.active / "broken.json"
        corrupt_path.write_text("{")

        for record in expired_records:
            store.save_active(record)
        result = self.run_json("cleanup")
        self.assertEqual(
            {item["session_id"] for item in result["archived"]}, expired_ids
        )
        self.assertEqual(result["corrupt"][0]["file"], "broken.json")
        self.assertEqual(corrupt_path.read_text(), "{")
        self.assertEqual(fresh_path.read_bytes(), fresh_bytes)
        history, corrupt = store.load_history()
        self.assertEqual(corrupt, [])
        self.assertEqual({record["session_id"] for record in history}, expired_ids)
        for record in history:
            self.assertEqual(record["result"], "abandoned")
            self.assertFalse((store.active / f"{record['session_id']}.json").exists())
        original_history = {
            path: path.read_bytes() for path in store.history.glob("*/*.json")
        }
        self.assertEqual(self.run_json("cleanup")["archived"], [])
        self.assertEqual(
            {path: path.read_bytes() for path in original_history}, original_history
        )

    def test_cleanup_expiry_boundary_and_interrupted_archive_retry(self) -> None:
        record = self.run_json("start", "--task", "expiry", "--scope", "expiry")[
            "record"
        ]
        store = coord.StateStore(self.repo / ".agent-coordination")
        expires = coord.parse_time(record["lease_expires_at"])
        with patch.object(
            coord, "utc_now", return_value=expires - timedelta(seconds=1)
        ):
            self.assertEqual(coord.cleanup_command(store)["archived"], [])
        active_path = store.active / f"{record['session_id']}.json"
        unlink = Path.unlink

        def interrupt(path, *arguments, **keywords):
            if path == active_path:
                raise OSError("simulated cleanup failure")
            return unlink(path, *arguments, **keywords)

        with (
            patch.object(coord, "utc_now", return_value=expires),
            patch.object(Path, "unlink", interrupt),
            self.assertRaisesRegex(OSError, "simulated cleanup failure"),
        ):
            coord.cleanup_command(store)
        history_path = next(store.history.glob("*/*.json"))
        original = history_path.read_bytes()
        with patch.object(coord, "utc_now", return_value=expires + timedelta(days=1)):
            result = coord.cleanup_command(store)
        self.assertEqual(len(result["archived"]), 1)
        self.assertFalse(active_path.exists())
        self.assertEqual(history_path.read_bytes(), original)

    def test_rejects_scope_outside_repository(self) -> None:
        completed = self.run_coord(
            "start", "--task", "unsafe", "--scope", "../outside", check=False
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("scope must stay within the repository", completed.stderr)

    def test_status_script_shows_active_and_historical_task_titles(self) -> None:
        historical = self.run_json(
            "start",
            "--task",
            "finished task",
            "--scope",
            "apps/finished",
        )
        self.run_json(
            "finish",
            "--id",
            str(historical["record"]["session_id"]),
            "--result",
            "completed",
        )
        self.run_json(
            "start",
            "--task",
            "active task",
            "--scope",
            "apps/active",
        )

        completed = subprocess.run(
            [sys.executable, str(STATUS_SCRIPT), "--repo", str(self.repo)],
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("active task", completed.stdout)
        self.assertIn("finished task", completed.stdout)


if __name__ == "__main__":
    unittest.main()
