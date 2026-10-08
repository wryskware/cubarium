"""Offline preparation and staged recovery mechanisms; no AWS, SSH or simulations."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts/studies"))
import batch
import eco_search
from study_contract import sha256

loader = importlib.util.spec_from_file_location("graviton", ROOT / "scripts/graviton-studies.py")
graviton = importlib.util.module_from_spec(loader)
loader.loader.exec_module(graviton)


class Preparation(unittest.TestCase):
    def test_plan_cannot_launch_and_accounts_for_baseline(self):
        with patch.object(graviton.subprocess, "run", side_effect=AssertionError("external command")):
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                graviton.main(["plan", "--spec", str(ROOT / "config/training/graviton-eco.json"),
                               "--price-per-host-hour", "0.75"])
            self.assertIn("136 worlds/generation; 3 waves", output.getvalue())
            self.assertIn("--resume", output.getvalue())
            self.assertIn("$9.00", output.getvalue())

    def test_kit_local_assembly_and_architecture_refusal(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            header = bytearray(20)
            header[:6] = b"\x7fELF\x02\x01"
            header[18:20] = (183).to_bytes(2, "little")
            for name in ("cubarium-search", "voxel_census"):
                (root / name).write_bytes(header)
            with contextlib.redirect_stdout(io.StringIO()):
                graviton.assemble(root / "kit", root, [])
            manifest = json.loads((root / "kit/kit.json").read_text())
            self.assertIn("scripts/studies/study_contract.py", manifest)
            self.assertIn("config/training/fitness-v2.toml", manifest)
            self.assertEqual(manifest["bin/voxel_census"], sha256(root / "voxel_census"))
            (root / "voxel_census").write_bytes(b"not ARM")
            with self.assertRaisesRegex(ValueError, "aarch64 ELF"):
                graviton.assemble(root / "bad", root, [])

    def test_checkpoint_selection_tracks_only_latest_and_committed_slot(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for minute in (1, 2):
                directory = root / f"arm/s1/run/ckpt/m{minute:06}"
                directory.mkdir(parents=True)
                for name in ("minute", "world.bin", "flora.bin", "fauna.bin"):
                    (directory / name).write_text(str(minute))
            (root / "checkpoint.json").write_text("checkpoint-b")
            for slot in ("checkpoint-a", "checkpoint-b"):
                directory = root / slot
                directory.mkdir()
                (directory / "state.json").write_text("{}")
            snapshot = graviton.checkpoint_snapshot(root)
            self.assertIn("m000002/***", snapshot["filters"])
            self.assertNotIn("m000001/***", snapshot["filters"])
            self.assertIn("/checkpoint-b/***", snapshot["filters"])
            self.assertNotIn("/checkpoint-a/***", snapshot["filters"])
            (root / "checkpoint-b/state.json").write_text('{"generation": 2}')
            self.assertNotEqual(snapshot, graviton.checkpoint_snapshot(root))

    def test_live_command_is_affined_and_launch_limits_cannot_be_overridden(self):
        job = dict(name="live", kind="live", args=["--centre", "browser=config/a.json"])
        command = graviton.command(job, "/kit", 57, 1, resume=True)
        self.assertEqual(command[:5], [graviton.PYTHON, "/kit/scripts/run-study.py", "--slot", "0", "--"])
        self.assertEqual(command[command.index("--threads") + 1], "16")
        self.assertEqual(command[-2:], ["--resume", "/kit/runs/live"])
        with tempfile.TemporaryDirectory() as tmp:
            spec = Path(tmp) / "spec.json"
            job["args"] += ["--threads=64"]
            spec.write_text(json.dumps(dict(jobs=[job])))
            with self.assertRaisesRegex(ValueError, "launcher-owned"):
                graviton.read_spec(spec)

    def test_rsync_filters_exclude_old_and_uncommitted_checkpoints(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source, dest = root / "source", root / "dest"
            source.mkdir()
            dest.mkdir()
            for minute in (1, 2):
                directory = source / f"arm/s1/run/ckpt/m{minute:06}"
                directory.mkdir(parents=True)
                for name in ("minute", "world.bin", "flora.bin", "fauna.bin"):
                    (directory / name).write_text(str(minute))
            (source / "checkpoint.json").write_text("checkpoint-b")
            for slot in ("checkpoint-a", "checkpoint-b"):
                (source / slot).mkdir()
                (source / slot / "state.json").write_text("{}")
            include = root / "filters"
            include.write_text(graviton.checkpoint_files(source))
            subprocess.run(["rsync", "-a", f"--include-from={include}", "--exclude=**/ckpt/***",
                            "--exclude=checkpoint-a/***", "--exclude=checkpoint-b/***",
                            f"{source}/", f"{dest}/"], check=True)
            self.assertTrue((dest / "arm/s1/run/ckpt/m000002/world.bin").exists())
            self.assertFalse((dest / "arm/s1/run/ckpt/m000001").exists())
            self.assertTrue((dest / "checkpoint-b/state.json").exists())
            self.assertFalse((dest / "checkpoint-a").exists())
            self.assertEqual(graviton.checkpoint_snapshot(source), graviton.checkpoint_snapshot(dest))


class Recovery(unittest.TestCase):
    def test_batch_resume_preserves_done_and_failed_and_uses_checkpoint(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            binary = out / "binary"
            binary.write_bytes(b"binary")
            jobs = []
            for seed in (1, 2, 3, 4):
                run = out / f"base/s{seed}/run"
                run.mkdir(parents=True)
                jobs.append(dict(arm="base", seed=seed, slot=seed-1, state="running",
                    command=["python3", "wrapper", "--slot", str(seed-1), "--", "chrt", "-i", "0",
                             "nice", "-n", "19", str(binary), "2", "threads=1"]))
                if seed == 1:
                    (run / "DONE").write_text("done")
                elif seed == 2:
                    checkpoint = run / "ckpt/m000003"
                    checkpoint.mkdir(parents=True)
                    (checkpoint / "minute").write_text("3")
                elif seed == 4:
                    (run / "FAILED").write_text("liveness failed")
            manifest = dict(binary_sha256=sha256(binary), identity=dict(config=None, required=None,
                            policies={}, starts={}), runs=jobs)
            (out / "batch.json").write_text(json.dumps(manifest))
            with patch.object(batch, "execute") as execute:
                batch.resume(out, binary, Path("wrapper"), 2, [[0, 1]], 1, 3)
            resumed = execute.call_args.args[1]
            self.assertEqual([j["state"] for j in resumed], ["done", "queued", "queued", "failed"])
            self.assertIn(f"resume={out}/base/s2/run", resumed[1]["command"])
            self.assertFalse((out / "base/s3").exists())
            self.assertTrue((out / "base/s1/run/DONE").exists())
            binary.write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "binary differs"):
                batch.resume(out, binary, Path("wrapper"), 2, [[0, 1]], 1, 3)

    def test_eco_resume_keeps_interrupted_batch(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            directory = out / "gens/g000"
            directory.mkdir(parents=True)
            (directory / "batch.json").write_text("{}")
            (directory / "STATE.tsv").write_text("arm\tseed\tstate\tminute\nbase\t1\tdone\t120\n")
            state = dict(args=dict(bin="binary", hours=2, threads=1, max_parallel=2,
                                   required="required", stage="check_min=3", config="config"), wall_used=0)
            with (out / "log").open("w") as log:
                search = eco_search.Search(out, state, log)
                with patch.object(eco_search.subprocess, "run") as run:
                    run.return_value.returncode = 0
                    rows = search.run_batch(directory, {"base": []}, [1])
                self.assertIn("--resume", run.call_args.args[0])
                self.assertEqual(rows[("base", 1)], "done")
                self.assertTrue((directory / "batch.json").is_file())


if __name__ == "__main__":
    unittest.main()
