"""Off-loop staging, resume subsets, transport failures and the optional S3 path.

Laws with fakes, no tuned numbers: a recording (or blocking, or failing) transport around the
local one, a localhost stand-in for the remote hosts and the EC2 instance, and a fake `aws`
cli whose buckets are directories (tests/fake_aws.py). Nothing reaches AWS or ssh.
"""

import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import threading
import time
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import test_lab
from labkit import transfer
from labkit.core import Kind, Task, register
from labkit.daemon import safe_key
from labkit.launch import LocalTransport

HERE = Path(__file__).resolve().parent
BUCKET = "test-bucket"
CHECKPOINT = (
    'mk() { d="$1/ckpt/m$2"; mkdir -p "$d.partial"; for f in world.bin flora.bin fauna.bin; do '
    'echo "$2" > "$d.partial/$f"; done; expr "$2" + 0 > "$d.partial/minute"; mv "$d.partial" "$d"; }; ')


class Recording(LocalTransport):
    """The local transport, recording every put; it can block on a gate or fail N times."""

    def __init__(self):
        self.calls = []
        self.gate = None
        self.failures = 0

    def _record(self, *call):
        self.calls.append(call)
        if self.gate is not None:
            self.gate.wait(30)
        if self.failures:
            self.failures -= 1
            raise subprocess.TimeoutExpired(["rsync"], call[-1])

    def put(self, local, remote, timeout=600):
        self._record("put", str(local), str(remote), timeout)
        super().put(local, remote, timeout)

    def put_dir(self, local, remote, timeout=600):
        self._record("put_dir", str(local), str(remote), timeout)
        super().put_dir(local, remote, timeout)

    def put_files(self, local, remote, files, timeout=600):
        self._record("put_files", str(local), str(remote), list(files), timeout)
        super().put_files(local, remote, files, timeout)

    def sent(self, root):
        """Every file sent from under root, relative to it (a put_dir counts its whole tree)."""
        root = str(root)
        out = set()
        for call in self.calls:
            if call[0] == "put_files" and call[1] == root:
                out.update(call[3])
            elif call[0] in ("put", "put_dir") and call[1].startswith(root):
                base = Path(call[1])
                paths = [base] if base.is_file() else [p for p in base.rglob("*") if p.is_file()]
                out.update(str(p.relative_to(root)) for p in paths)
        return out


@register
class Resumable(Kind):
    """One task that resumes its own run dir; argv and placement come from the spec."""
    name = "test-transfer-resume"

    def plan(self, job, ctx):
        files = {job.spec["start"]: "{job}/start/ckpt/m000005"} if job.spec.get("start") else {}
        argv = ["sh", "-c", job.spec["script"]]
        return [Task(name="run", argv=argv, resume_argv=argv, hosts=job.spec["hosts"], files=files,
                     retries=1, inputs=job.spec.get("inputs", []))]


@register
class FakeBuild(Kind):
    """A build-shaped task (meta kind/key, produces) whose "compiler" logs each build."""
    name = "test-transfer-build"

    def plan(self, job, ctx):
        key = job.spec["key"]
        script = f'echo built >> {job.spec["log"]}; mkdir -p {{out}}/artifact; printf %s {job.spec["content"]} > {{out}}/artifact/tool'
        return [Task(name="build", argv=["sh", "-c", script], hosts=["ec2"], produces={key: "artifact/tool"},
                     meta=dict(kind="build", key=key))]


def checkpoint(run, minute, layers=True):
    path = Path(run) / "ckpt" / f"m{minute:06d}"
    path.mkdir(parents=True)
    (path / "minute").write_text(f"{minute}\n")
    for name in transfer.LAYERS if layers else ():
        (path / name).write_bytes(name.encode() * 64)
    return path


class TransferCase(test_lab.LabCase):
    def setUp(self):
        super().setUp()
        self.data = self.inventory()
        self.data["lab"].update(transport_backoff_s=30, upload_floor_mb_s=0.001)
        self.data["hosts"]["far"] = dict(kind="ssh", address="local", arch=platform.machine(), cpus=2,
                                         groups="auto", max_cpu_fraction=1.0, reserve_cpus=0, max_mem_gb=16,
                                         launcher="setsid", root=str(self.root / "far"), python=sys.executable,
                                         idle=True, external_load=False)

    def ready_outputs(self, job_id, name="run"):
        """A previous attempt's outputs: three complete checkpoints, a .partial and a cut-short one."""
        out = self.paths.runs / job_id / name
        run = out / "run"
        for minute in (10, 20, 30):
            checkpoint(run, minute)
        checkpoint(run, 40).rename(run / "ckpt/m000040.partial")
        cut = checkpoint(run, 50)
        (cut / "world.bin").unlink()     # rsync sends `minute` before world.bin
        (run / "args").write_text("binary\nseed=1\n")
        (run / "status").write_text("minute=52 end=100\n")
        (out / "census.csv").write_text("sim_min,count\n10,1\n20,1\n30,1\n")
        (out / "vitals").mkdir()
        (out / "vitals/vitals.csv").write_text("minute\n30\n")
        return out


class StagingTests(TransferCase):
    def test_tick_completes_while_a_remote_upload_blocks(self):
        daemon = self.daemon(self.data)
        recording = Recording()
        recording.gate = threading.Event()
        self.addCleanup(recording.gate.set)
        daemon.transports["far"] = recording
        remote = self.submit(dict(kind="command", hosts=["far"], argv=["true"]))
        test_lab.wait_for(lambda: self.events("staging", remote), step=daemon.step)
        local = self.submit(dict(kind="command", hosts=["fake"], argv=["sh", "-c", "echo local > {out}/result"]))
        self.assertEqual(self.finished(daemon, local)[0], "done")
        # The upload is still blocked: the loop polled, launched and finished the local task.
        self.assertFalse(recording.gate.is_set())
        self.assertEqual(daemon.job(remote).tasks["run"].state, "pending")
        self.assertIn((remote, "run"), daemon.staging)
        self.assertEqual(self.events("started", remote), [])
        self.assertTrue(self.events("task-done", local))
        recording.gate.set()
        self.assertEqual(self.finished(daemon, remote)[0], "done")
        started = self.events("started", remote)
        self.assertEqual([e["host"] for e in started], ["far"])
        self.assertGreater(started[0]["seq"], self.events("done", local)[0]["seq"])

    def test_resume_puts_only_the_newest_complete_checkpoint(self):
        daemon = self.daemon(self.data)
        recording = Recording()
        daemon.transports["far"] = recording
        job_id = "resume-subset"
        out = self.ready_outputs(job_id)
        start = checkpoint(self.root / "library/run", 5)
        script = 'test -f {out}/run/ckpt/m000030/fauna.bin && test -f {out}/census.csv && test -f {out}/run/args'
        self.submit(dict(kind=Resumable.name, hosts=["far"], script=script, start=str(start)), job_id)
        self.assertEqual(self.finished(daemon, job_id)[0], "done")
        sent = recording.sent(out)
        checkpoints = {p for p in sent if "/ckpt/" in p}
        self.assertEqual(checkpoints, {f"run/ckpt/m000030/{n}" for n in ("minute", *transfer.LAYERS)})
        self.assertTrue({"run/args", "run/status", "census.csv", "vitals/vitals.csv"} <= sent)
        # The checkpoint it started from stays home: its own checkpoint resumes it.
        self.assertFalse(any(call[1].startswith(str(start.parent)) for call in recording.calls))
        # Upload timeouts follow the bytes at the configured floor rate.
        for call in recording.calls:
            if call[0] == "put_files":
                nbytes = transfer.size_of(call[1], call[3])
                self.assertGreaterEqual(call[4], nbytes / (self.data["lab"]["upload_floor_mb_s"] * 1e6))

    def test_upload_timeout_requeues_without_a_retry_and_backs_off(self):
        daemon = self.daemon(self.data, clock=lambda: self.now[0])
        recording = Recording()
        recording.failures = 2
        daemon.transports["far"] = recording
        job = self.submit(dict(kind="command", hosts=["far"], argv=["true"]))
        failed = test_lab.wait_for(lambda: self.events("stage-failed", job), step=daemon.step)
        row = daemon.store.one("SELECT * FROM tasks WHERE job=?", job)
        self.assertEqual((row["state"], row["attempts"]), ("pending", 0))
        self.assertEqual(failed[0]["attempts"], 0)
        calls = len(recording.calls)
        for _ in range(20):          # inside the backoff nothing is retried
            daemon.step()
        self.assertEqual(len(recording.calls), calls)
        self.assertEqual(len(self.events("staging", job)), 1)
        self.now[0] += failed[0]["retry_in_s"] + 1
        second = test_lab.wait_for(lambda: self.events("stage-failed", job)[1:], step=daemon.step)
        self.assertGreater(second[0]["retry_in_s"], failed[0]["retry_in_s"])
        self.now[0] += second[0]["retry_in_s"] + 1
        self.assertEqual(self.finished(daemon, job)[0], "done")
        row = daemon.store.one("SELECT * FROM tasks WHERE job=?", job)
        self.assertEqual(row["attempts"], 1)
        self.assertEqual(len(self.events("started", job)), 1)

    def test_restart_mid_upload_restages_and_never_half_launches(self):
        first = self.daemon(self.data)
        recording = Recording()
        recording.gate = threading.Event()
        self.addCleanup(recording.gate.set)
        first.transports["far"] = recording
        job = self.submit(dict(kind="command", hosts=["far"], argv=["true"]))
        test_lab.wait_for(lambda: self.events("staging", job), step=first.step)
        second = self.daemon(self.data)     # the first one "died" mid-upload
        self.assertEqual(second.job(job).tasks["run"].state, "pending")
        self.assertEqual(self.finished(second, job)[0], "done")
        # The dead daemon's upload finishing late changes nothing: the task already ran once.
        recording.gate.set()
        test_lab.wait_for(lambda: not first.done.empty())
        first.drain_transfers()
        self.assertEqual(len(self.events("started", job)), 1)


class S3Case(TransferCase):
    def setUp(self):
        super().setUp()
        self.bucket_root = self.root / "s3"
        (self.bucket_root / BUCKET).mkdir(parents=True)
        self.aws_log = self.root / "aws.log"
        bindir = self.root / "aws-bin"
        bindir.mkdir()
        aws = bindir / "aws"
        aws.write_text(f"#!{sys.executable}\nimport runpy, sys\nsys.argv[0] = {str(HERE / 'fake_aws.py')!r}\n"
                       f"runpy.run_path(sys.argv[0], run_name='__main__')\n")
        aws.chmod(0o755)
        env = patch.dict(os.environ, {"PATH": f"{bindir}{os.pathsep}{os.environ['PATH']}",
                                      "FAKE_AWS_ROOT": str(self.bucket_root), "FAKE_AWS_LOG": str(self.aws_log)})
        env.start()
        self.addCleanup(env.stop)
        script = self.root / "aws-graviton.sh"
        script.write_text(f'#!/bin/sh\necho "$@" >> {self.root}/graviton.log\ncase "$1" in\n'
                          f'  up) touch {self.root}/instance; echo i-0abc123 ;;\n'
                          f'  status) [ ! -f {self.root}/instance ] || echo "1 i-0abc123 127.0.0.1 running spot now mock" ;;\n'
                          f'  down) rm -f {self.root}/instance ;;\nesac\nexit 0\n')
        script.chmod(0o755)
        self.data["ec2"] = dict(script=str(script), types=["mock"], transport="local", root=str(self.root / "ec2root"),
                                ready_file="", launcher="setsid", python=sys.executable, status_s=0,
                                max_cpu_fraction=1.0, reserve_cpus=0, s3_bucket=BUCKET, s3_region="us-east-1",
                                instance_types=dict(mock=dict(arch=platform.machine(), cpus=4, mem_gb=8,
                                                              spot_usd_h=0.1)))

    def bring_up(self, daemon, root=None):
        """The instance is up (the approval flow has its own tests): probe it like host-up does."""
        pool = daemon.ec2
        if root:
            pool.cfg["root"] = str(root)
        (self.root / "instance").touch()
        pool.forget_host()
        pool.s = dict(state="up", type="mock", instance="i-0abc123", launched=daemon.clock(), max_hours=10,
                      max_usd=100, ip="127.0.0.1")
        pool.host = pool.make_host("127.0.0.1")
        self.assertTrue(pool.probe_host())
        pool.probe_s3()
        pool.save()
        return pool

    def aws_calls(self):
        if not self.aws_log.exists():
            return []
        return [json.loads(line) for line in self.aws_log.read_text().splitlines()]

    def pulls(self):
        return [c for c in self.aws_calls() if "cp" in c and any(a.startswith(f"s3://{BUCKET}/ckpt/") for a in c[-2:-1])]

    def s3_files(self, prefix):
        base = self.bucket_root / BUCKET / prefix
        return sorted(str(p.relative_to(base)) for p in base.rglob("*") if p.is_file()) if base.is_dir() else []


class S3CheckpointTests(S3Case):
    def test_capable_resume_pulls_from_s3_and_puts_no_checkpoint(self):
        daemon = self.daemon(self.data)
        self.bring_up(daemon)
        self.assertTrue(self.events("ec2-s3")[-1]["capable"])
        gate = self.root / "gate"
        script = (CHECKPOINT + 'run={out}/run; if [ -f "$run/ckpt/m000030/minute" ]; then echo ok > {out}/resumed; exit 0; fi; '
                  'mkdir -p "$run"; printf "binary\\nseed=1\\n" > "$run/args"; echo sim_min,count > {out}/census.csv; '
                  'for m in 000010 000020 000030; do mk "$run" $m; echo "$(expr $m + 0),1" >> {out}/census.csv; done; '
                  f'while [ ! -f {gate} ]; do sleep 0.05; done')
        job = self.submit(dict(kind=Resumable.name, hosts=["ec2"], script=script), "s3-resume")
        mark = f"s3ckpt:{job}:run"
        test_lab.wait_for(lambda: daemon.store.kv_get(mark, {}).get("run") == "m000030", step=daemon.step)
        # Only the newest checkpoint is kept under the task's prefix.
        self.assertEqual(self.s3_files(f"ckpt/{job}/run/run"),
                         sorted(f"m000030/{n}" for n in ("minute", *transfer.LAYERS)))
        self.assertTrue(self.events("s3-checkpoint", job))
        row = daemon.store.one("SELECT * FROM tasks WHERE job=?", job)
        daemon.launchers["ec2"].stop(row["unit"], daemon.lab_dir(row))
        test_lab.wait_for(lambda: (Path(daemon.lab_dir(row)) / "rc.json").is_file())
        daemon.task_lost(row, "simulated loss")
        recording = Recording()
        daemon.transports["ec2"] = recording
        self.aws_log.unlink()
        self.assertEqual(self.finished(daemon, job)[0], "done")
        out = daemon.job(job).tasks["run"].out
        self.assertEqual((out / "resumed").read_text(), "ok\n")
        self.assertFalse({p for p in recording.sent(out) if "/ckpt/" in p})
        self.assertTrue(recording.sent(out))      # args and census still go from home
        self.assertEqual(len(self.pulls()), 1)

    def test_home_newest_goes_up_when_s3_lacks_it_or_is_older(self):
        for case in ("lacks", "older"):
            with self.subTest(case=case):
                daemon = self.daemon(self.data)
                self.bring_up(daemon)
                job_id = f"s3-{case}"
                out = self.ready_outputs(job_id)
                if case == "older":
                    stored = self.bucket_root / BUCKET / f"ckpt/{job_id}/run/run/m000020"
                    stored.parent.mkdir(parents=True)
                    subprocess.run(["cp", "-a", str(out / "run/ckpt/m000020"), str(stored)], check=True)
                    daemon.store.kv_set(f"s3ckpt:{job_id}:run", {"run": "m000020"})
                recording = Recording()
                daemon.transports["ec2"] = recording
                if self.aws_log.exists():
                    self.aws_log.unlink()
                script = "test -f {out}/run/ckpt/m000030/fauna.bin"
                self.submit(dict(kind=Resumable.name, hosts=["ec2"], script=script), job_id)
                self.assertEqual(self.finished(daemon, job_id)[0], "done")
                checkpoints = {p for p in recording.sent(out) if "/ckpt/" in p}
                self.assertEqual(checkpoints, {f"run/ckpt/m000030/{n}" for n in ("minute", *transfer.LAYERS)})
                self.assertEqual(self.pulls(), [])

    def test_no_s3_command_without_a_bucket_or_without_access(self):
        for case in ("off", "denied"):
            with self.subTest(case=case):
                data = json.loads(json.dumps(self.data))
                if case == "off":
                    del data["ec2"]["s3_bucket"]
                else:
                    (self.bucket_root / BUCKET / ".deny").touch()
                if self.aws_log.exists():
                    self.aws_log.unlink()
                daemon = self.daemon(data)
                self.bring_up(daemon)
                job_id = f"s3-{case}"
                out = self.ready_outputs(job_id)
                key = daemon.ctx.binary(str(self.root), "0" * 40, arch=platform.machine())
                tool = self.root / f"tool-{case}"
                tool.write_text("#!/bin/sh\n")
                daemon.register_artifact(key, tool, "fixture")
                recording = Recording()
                daemon.transports["ec2"] = recording
                script = "test -f {out}/run/ckpt/m000030/fauna.bin"
                self.submit(dict(kind=Resumable.name, hosts=["ec2"], script=script, inputs=[key]), job_id)
                self.assertEqual(self.finished(daemon, job_id)[0], "done")
                self.assertTrue({p for p in recording.sent(out) if "/ckpt/m000030/" in p})
                if case == "off":
                    self.assertEqual(self.aws_calls(), [])
                    self.assertEqual(self.events("ec2-s3"), [])
                else:
                    # The host-up probe is the only S3 command: everything else stays off.
                    self.assertEqual([c[c.index("s3") + 1] for c in self.aws_calls()], ["ls"])
                    self.assertFalse(self.events("ec2-s3")[-1]["capable"])
                    (self.bucket_root / BUCKET / ".deny").unlink()


class S3BinaryTests(S3Case):
    def test_second_instance_fetches_the_binary_and_a_mismatch_builds(self):
        daemon = self.daemon(self.data)
        key = daemon.ctx.binary(str(self.root), "1" * 40, arch=platform.machine())
        log = self.root / "builds.log"
        stored = self.bucket_root / BUCKET / "bin" / safe_key(key)

        def build(instance, content="native"):
            self.bring_up(daemon, self.root / f"ec2root-{instance}")
            job = self.submit(dict(kind=FakeBuild.name, key=key, log=str(log), content=content))
            self.assertEqual(self.finished(daemon, job)[0], "done")
            return daemon.ctx.artifact(key).read_text()

        self.assertEqual(build(1), "native")
        self.assertEqual(log.read_text().count("built"), 1)
        self.assertEqual(sorted(p.name for p in stored.iterdir()), ["sha256", "tool"])
        # A new instance, same key: fetched and checksum-verified, no build.
        self.assertEqual(build(2, content="rebuilt"), "native")
        self.assertEqual(log.read_text().count("built"), 1)
        # The S3 copy no longer matches its checksum: the instance builds (and pushes again).
        (stored / "tool").write_text("corrupt")
        self.assertEqual(build(3, content="rebuilt"), "rebuilt")
        self.assertEqual(log.read_text().count("built"), 2)
        self.assertEqual((stored / "tool").read_text(), "rebuilt")

    def test_binary_input_comes_from_s3_when_it_matches_home(self):
        daemon = self.daemon(self.data)
        key = daemon.ctx.binary(str(self.root), "2" * 40, arch=platform.machine())
        tool = self.root / "voxel_census"
        tool.write_text("#!/bin/sh\necho home binary\n")
        tool.chmod(0o755)
        daemon.register_artifact(key, tool, "fixture")
        home = daemon.ctx.artifact(key)
        for instance, expect_put in ((1, True), (2, False)):
            with self.subTest(instance=instance):
                self.bring_up(daemon, self.root / f"ec2root-{instance}")
                recording = Recording()
                daemon.transports["ec2"] = recording
                script = "test -x {input:%s}" % key
                job = self.submit(dict(kind=Resumable.name, hosts=["ec2"], script=script, inputs=[key]))
                self.assertEqual(self.finished(daemon, job)[0], "done")
                put = [c for c in recording.calls if c[0] == "put" and c[1] == str(home)]
                self.assertEqual(bool(put), expect_put)


class GravitonProfileTests(S3Case):
    def test_up_passes_the_instance_profile_only_when_named(self):
        for profile in (None, "cubarium-lab-ec2"):
            with self.subTest(profile=profile):
                data = json.loads(json.dumps(self.data))
                if profile:
                    data["ec2"]["instance_profile"] = profile
                daemon = self.daemon(data)
                log = self.root / "graviton.log"
                log.unlink(missing_ok=True)
                daemon.ec2.s.update(type="mock", max_hours=1, max_usd=1, approval="test")
                daemon.ec2.launch()
                up = log.read_text().splitlines()[0]
                if profile:
                    self.assertIn(f"--iam-instance-profile {profile}", up)
                else:
                    self.assertNotIn("--iam-instance-profile", up)

    def test_graviton_script_parses(self):
        script = Path(__file__).resolve().parents[2] / "aws-graviton.sh"
        done = subprocess.run(["bash", "-n", str(script)], capture_output=True, text=True, timeout=10)
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("--iam-instance-profile", script.read_text())


class ResumePlanTests(test_lab.LabCase):
    def test_plan_keeps_each_batch_childs_newest_and_everything_small(self):
        out = self.root / "task"
        for child, minutes in (("batch/a/s1/run", (20, 40)), ("batch/b/s2/run", (20,))):
            for minute in minutes:
                checkpoint(out / child, minute)
            (out / child / "args").write_text("x\n")
        cut = checkpoint(out / "batch/a/s1/run", 60)
        (cut / "flora.bin").unlink()
        (out / "batch/batch.json").write_text("{}")
        files, checkpoints, runs = transfer.resume_plan(out)
        self.assertEqual(runs, ["batch/a/s1/run", "batch/b/s2/run"])
        self.assertEqual({rel: item[1] for rel, item in checkpoints.items()},
                         {"batch/a/s1/run": "m000040", "batch/b/s2/run": "m000020"})
        self.assertIn("batch/batch.json", files)
        self.assertFalse(any("/ckpt/" in f for f in files))
