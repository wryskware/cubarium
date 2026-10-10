"""Remote jobs on a localhost transport; all state and children are test-owned."""

import json
import os
from pathlib import Path
import platform
import subprocess
import sys
from unittest.mock import patch
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import test_lab
from labkit.core import Abort, Kind, Task, register
from labkit.hosts import fits, Usage
from labkit.kinds.staging import pinned_config
from labkit.launch import SshTransport


@register
class Observe(Kind):
    name = "test-remote-observe"

    def plan(self, job, ctx):
        return [Task(name="run", argv=["sh", "-c", job.spec["script"]], hosts=["far"])]

    def progress(self, job, task, ctx):
        if (job.tasks["run"].out / "census.csv").is_file():
            job.state["observed"] = (job.tasks["run"].out / "census.csv").read_text()
        return super().progress(job, task, ctx)


class RemoteTests(test_lab.LabCase):
    def setUp(self):
        super().setUp()
        self.trees, self.binary = test_lab.test_ab.make_fixture(self.root)
        self.data = self.inventory()
        self.data["hosts"]["fake"].update(groups="live")
        del self.data["hosts"]["fake"]["cpus"]
        self.data["hosts"]["far"] = dict(kind="ssh", address="local", arch=platform.machine(), cpus=2,
                                         groups="auto", max_cpu_fraction=1.0, reserve_cpus=0, max_mem_gb=16,
                                         launcher="setsid", root=str(self.root / "far"), python=sys.executable,
                                         idle=True, external_load=False)

    def change(self):
        test_lab.ABKindTests.commit_change(self)

    def ab_spec(self, **extra):
        spec = test_lab.ABKindTests.spec(self, preflight_only=False, hosts=["far"], expect=["fauna.rate"],
                                       expect_metric=["count"], hours=0.4)
        spec.update(extra)
        return spec

    def run_ab(self, **extra):
        daemon = self.daemon(self.data)
        job = self.submit(self.ab_spec(**extra))
        return daemon, job, self.finished(daemon, job)

    def test_ab_stages_cached_binary_pinned_config_and_own_checkpoint(self):
        self.change()
        self.binary.write_text(self.binary.read_text().replace('touch "$run/DONE"',
                               'touch "$run/DONE"\nif [ -n "$load" ]; then cp "$load/minute" "$run/loaded-minute"; '
                               'else cp "$config" "$run/config-used"; fi'))
        daemon = self.daemon(self.data)
        spec = self.ab_spec()
        for arm in spec["arms"].values():
            del arm["bin"]
            commit = subprocess.check_output(["git", "-C", arm["tree"], "rev-parse", "HEAD"], text=True).strip()
            key = daemon.ctx.binary(arm["tree"], commit)
            daemon.register_artifact(key, self.binary, "fixture")
        job = self.submit(spec)
        self.assertEqual(self.finished(daemon, job)[0], "done")
        views = daemon.job(job).tasks
        for view in views.values():
            self.assertEqual(view.host, "far")
            launch = json.loads((daemon.job(job).out / ".lab" / view.name / "launch.json").read_text())
            self.assertIn("/far/artifacts/", launch["argv"][0])
            self.assertTrue(launch["idle"])
            self.assertIsNotNone(launch["group"])
            self.assertIsNone(launch["cpus"])
            self.assertTrue((view.out / "census.csv").is_file())
            self.assertTrue((view.out / "run/ckpt/m000020/minute").is_file())
            self.assertFalse(Path(daemon.remote_out(daemon.host("far"), daemon.job(job), view)).exists())
            if view.task.meta["stage"] == "continuation":
                load = next(a for a in launch["argv"] if a.startswith("load="))
                self.assertIn(f"/early/{view.task.meta['arm']}/s1/", load)
                self.assertEqual((view.out / "run/loaded-minute").read_text(), "20\n")
            else:
                self.assertEqual((view.out / "run/config-used").read_bytes(),
                                 (self.trees[view.task.meta["arm"]] / "config/input.toml").read_bytes())

    def test_identical_remote_early_metrics_abort_before_continuation(self):
        self.change()
        self.binary.write_text(self.binary.read_text().replace("count=8", "count=7"))
        daemon, job, (state, reason) = self.run_ab()
        self.assertEqual(state, "aborted")
        self.assertIn("early-identical", reason)
        self.assertTrue(all(v.host == "far" and v.task.meta["stage"] == "early" for v in daemon.job(job).tasks.values()))
        self.assertTrue((daemon.job(job).out / "early/main/s1/census.csv").is_file())

    def test_incremental_fetch_is_visible_to_progress_then_final_cleanup(self):
        daemon = self.daemon(self.data)
        gate = self.root / "gate"
        script = f"echo first > {{out}}/census.csv; while [ ! -f {gate} ]; do sleep 0.05; done; echo final >> {{out}}/census.csv"
        job = self.submit(dict(kind=Observe.name, script=script))
        test_lab.wait_for(lambda: daemon.job(job) and daemon.job(job).state.get("observed"), step=daemon.step)
        self.assertEqual(daemon.job(job).state["observed"], "first\n")
        self.assertEqual(daemon.job(job).tasks["run"].state, "running")
        remote = Path(daemon.remote_out(daemon.host("far"), daemon.job(job), daemon.job(job).tasks["run"]))
        self.assertTrue(remote.exists())
        gate.touch()
        self.assertEqual(self.finished(daemon, job)[0], "done")
        self.assertEqual((daemon.job(job).tasks["run"].out / "census.csv").read_text(), "first\nfinal\n")
        self.assertFalse(remote.exists())

    def test_final_fetch_failure_retains_remote_copy_until_retry(self):
        daemon = self.daemon(self.data)
        job = self.submit(dict(kind="command", hosts=["far"], argv=["sh", "-c", "echo result > {out}/result"]))
        transport = daemon.transports["far"]
        with patch.object(transport, "get", side_effect=OSError("offline")):
            test_lab.wait_for(lambda: self.events("started", job), step=daemon.step)
            row = daemon.store.one("SELECT * FROM tasks WHERE job=?", job)
            test_lab.wait_for(lambda: (Path(daemon.lab_dir(row)) / "rc.json").is_file())
            daemon.poll_tasks(force=True)
            self.assertEqual(daemon.job(job).tasks["run"].state, "running")
            self.assertTrue(Path(daemon.remote_out(daemon.host("far"), daemon.job(job), daemon.job(job).tasks["run"])).exists())
        self.assertEqual(self.finished(daemon, job)[0], "done")
        self.assertEqual((daemon.job(job).tasks["run"].out / "result").read_text(), "result\n")

    def test_restart_reconciles_remote_run_without_relaunch(self):
        first = self.daemon(self.data)
        gate = self.root / "gate"
        job = self.submit(dict(kind="command", hosts=["far"], argv=["sh", "-c",
                              f"echo started > {{out}}/result; while [ ! -f {gate} ]; do sleep 0.05; done; echo ended >> {{out}}/result"]))
        test_lab.wait_for(lambda: self.events("started", job), step=first.step)
        first.pool.shutdown(wait=True)
        second = self.daemon(self.data)
        self.assertEqual(second.job(job).tasks["run"].state, "running")
        gate.touch()
        self.assertEqual(self.finished(second, job)[0], "done")
        self.assertEqual(len(self.events("started", job)), 1)
        self.assertIn("ended", (second.job(job).tasks["run"].out / "result").read_text())

    def test_cancel_fetches_final_remote_logs_before_cleanup(self):
        daemon = self.daemon(self.data)
        gate = self.root / "gate"
        job = self.submit(dict(kind="command", hosts=["far"], argv=["sh", "-c",
                              f"echo stopped-output > {{out}}/result; while [ ! -f {gate} ]; do sleep 0.05; done"]))
        test_lab.wait_for(lambda: self.events("started", job), step=daemon.step)
        remote = Path(daemon.remote_out(daemon.host("far"), daemon.job(job), daemon.job(job).tasks["run"]))
        test_lab.wait_for(lambda: (remote / "result").is_file())
        daemon.cancel(job)
        self.assertEqual(self.finished(daemon, job)[0], "cancelled")
        self.assertEqual((daemon.job(job).tasks["run"].out / "result").read_text(), "stopped-output\n")
        self.assertFalse(remote.exists())

    def test_host_pins_and_most_free_threads_with_local_only(self):
        daemon = self.daemon(self.data)
        local, remote = daemon.host("fake"), daemon.host("far")
        local.external_load = True
        remote.external_load = True
        local.external_threads = local.usable - 2
        remote.external_threads = remote.usable - 3
        with patch.object(daemon, "local_load"):
            jobs = [self.submit(dict(kind="command", argv=["true"], **extra)) for extra in
                    ({}, {"hosts": ["fake"]}, {"hosts": ["far"]}, {"local_only": True})]
            for job in jobs:
                self.assertEqual(self.finished(daemon, job)[0], "done")
        self.assertEqual([daemon.job(job).tasks["run"].host for job in jobs], ["far", "fake", "far", "fake"])
        task = Task(name="local", argv=["true"], local_only=True, hosts=["far"])
        self.assertIsNone(fits(task, remote, Usage()))
        build = daemon.kinds["build"]
        spec = build.validate(dict(repo=str(self.trees["main"]), commit="HEAD"), daemon.ctx)
        job = type("Job", (), dict(spec=spec, state={}))()
        self.assertTrue(build.plan(job, daemon.ctx)[0].local_only)

    def test_pinned_config_dependencies_come_from_arm_commit(self):
        tree = self.trees["main"]
        (tree / "config/input.toml").write_text('include = "child.toml"\n[asset]\npath = "payload.bin"\n')
        (tree / "config/child.toml").write_text('value = "pinned"\n')
        (tree / "config/payload.bin").write_bytes(b"pinned payload")
        subprocess.run(["git", "-C", str(tree), "add", "config"], check=True)
        subprocess.run(["git", "-C", str(tree), "-c", "user.name=T", "-c", "user.email=t@x.invalid",
                        "commit", "-qm", "config dependencies"], check=True)
        commit = subprocess.check_output(["git", "-C", str(tree), "rev-parse", "HEAD"], text=True).strip()
        (tree / "config/child.toml").write_text('value = "live edit"\n')
        root = self.root / "bundle"
        pinned_config(tree, commit, "config/input.toml", root)
        self.assertEqual((root / "config/child.toml").read_text(), 'value = "pinned"\n')
        self.assertEqual((root / "config/payload.bin").read_bytes(), b"pinned payload")

    def test_cargo_test_uses_pinned_archive_warm_target_and_reports_exit(self):
        daemon = self.daemon(self.data)
        bindir = self.root / "bin"
        bindir.mkdir()
        cargo = bindir / "cargo"
        cargo.write_text('#!/bin/sh\nmkdir -p "$CARGO_TARGET_DIR"\ncat config/input.toml\n'
                         'test ! -f "$CARGO_TARGET_DIR/warm" || echo warm\ntouch "$CARGO_TARGET_DIR/warm"\n'
                         'echo test-log >&2\ntest "$1" != fail\n')
        cargo.chmod(0o755)
        with patch.dict(os.environ, {"PATH": str(bindir) + os.pathsep + os.environ["PATH"]}):
            jobs = []
            for command in (["cargo", "test"], ["cargo", "fail"]):
                jobs.append(self.submit(dict(kind="cargo-test", repo=str(self.trees["main"]), commit="HEAD",
                                             hosts=["far"], command=command, threads=1, mem_gb=1)))
                state, reason = self.finished(daemon, jobs[-1])
            first, second = [daemon.job(job) for job in jobs]
            self.assertEqual(first.tasks["test"].state, "done")
            self.assertEqual(second.tasks["test"].rc, 1)
            summary = daemon.store.one("SELECT summary FROM jobs WHERE id=?", second.id)["summary"]
            self.assertIn("FAILED", summary)
            self.assertIn("test-log", summary)
            self.assertIn("warm", (second.tasks["test"].out / "stdout.log").read_text())
            self.assertTrue((self.root / "far/target/warm").is_file())
            self.assertEqual(first.tasks["test"].host, "far")
            self.assertIn("rate=3", (first.tasks["test"].out / "stdout.log").read_text())
            providers = daemon.store.q("SELECT * FROM providers WHERE key LIKE 'source:%'")
            self.assertEqual(len(providers), 1)
            self.assertTrue(all(v.host == "fake" for v in daemon.job(providers[0]["job"]).tasks.values()))

    def test_ssh_fetch_uses_incremental_rsync(self):
        transport = SshTransport("fake.invalid")
        with patch("labkit.launch.subprocess.run") as run:
            transport.get("/remote/out", self.root / "mirror")
        argv = run.call_args.args[0]
        self.assertEqual(argv[:2], ["rsync", "-a"])
        self.assertIn("fake.invalid:/remote/out/", argv)

    def test_remote_batch_stages_config_starts_policies_and_resumes_on_either_host(self):
        daemon = self.daemon(self.data)
        self.binary.write_text(self.binary.read_text().replace('touch "$run/DONE"',
                               'touch "$run/DONE"\nif [ -n "$load" ]; then cp "$load/minute" "$run/loaded-minute"; '
                               'else cp "$config" "$run/config-used"; fi'))
        required = self.root / "required.toml"
        required.write_text('[required]\nanimals = []\nplants = ["fixture"]\n')
        policy = self.root / "policy.bin"
        policy.write_bytes(b"policy input")
        genome = self.root / "genome.toml"
        genome.write_text("gene = []\n")
        checkpoint = self.root / "library/s4/run/ckpt/m000020"
        checkpoint.mkdir(parents=True)
        (checkpoint / "minute").write_text("20\n")
        for name in ("world.bin", "flora.bin", "fauna.bin"):
            (checkpoint / name).write_bytes(name.encode())
        commit = subprocess.check_output(["git", "-C", str(self.trees["main"]), "rev-parse", "HEAD"], text=True).strip()
        key = daemon.ctx.binary(str(self.trees["main"]), commit)
        daemon.register_artifact(key, self.binary, "fixture")
        for source in (["--config=config/input.toml", "--seeds", "1"], ["--starts", str(checkpoint)]):
            job = self.submit(dict(kind="batch", hosts=["far"], parallel=1, mem_gb=1,
                                  binary=dict(tree=str(self.trees["main"]), commit=commit),
                                  _cwd=str(self.trees["main"]), args=["--arm", f"base=policy=fixture={policy} genome={genome}",
                                  "--required", str(required), "--hours", "0.1", *source]))
            self.assertEqual(self.finished(daemon, job)[0], "done")
            view = daemon.job(job).tasks["batch"]
            manifest = json.loads((view.out / "batch/batch.json").read_text())
            run = manifest["runs"][0]
            self.assertIn("\tdone\t", (view.out / "batch/STATE.tsv").read_text())
            self.assertEqual(view.host, "far")
            self.assertTrue(view.task.l3)
            self.assertIn("/far/jobs/", manifest["identity"]["required"]["path"])
            self.assertIn("/far/jobs/", manifest["identity"]["policies"]["base"]["fixture"]["path"])
            if source[0].startswith("--config"):
                self.assertEqual((view.out / "batch/base/s1/run/config-used").read_bytes(),
                                 (self.trees["main"] / "config/input.toml").read_bytes())
            else:
                self.assertEqual((view.out / "batch/base/s4/run/loaded-minute").read_text(), "20\n")
            self.assertFalse((self.root / "far/jobs" / job).exists())
            for host in ("fake", "far"):
                resumed = self.submit(dict(kind="batch", hosts=[host], parallel=1, mem_gb=1,
                                           binary=dict(tree=str(self.trees["main"]), commit=commit), resume=str(view.out / "batch")))
                self.assertEqual(self.finished(daemon, resumed)[0], "done")
                resumed_view = daemon.job(resumed).tasks["batch"]
                resumed_manifest = json.loads((resumed_view.out / "batch/batch.json").read_text())
                self.assertEqual(resumed_view.host, host)
                self.assertEqual(resumed_manifest["runs"][0]["state"], "done")
                if host == "fake":
                    self.assertNotIn("/far/", resumed_manifest["identity"]["required"]["path"])
                # Completed output must travel with an explicit remote resume, too.
                seed = 1 if source[0].startswith("--config") else 4
                self.assertTrue((resumed_view.out / f"batch/base/s{seed}/run/DONE").is_file())

    def test_lost_remote_batch_retries_from_fetched_checkpoint_on_local_host(self):
        daemon = self.daemon(self.data)
        gate = self.root / "gate"
        script = self.binary.read_text().replace('    load=*) load=${arg#load=} ;;',
                 '    load=*) load=${arg#load=} ;;\n    resume=*) run=${arg#resume=}; load=$run/ckpt/m000020 ;;')
        script = script.replace('touch "$run/DONE"',
                 f'if [ -z "$load" ]; then while [ ! -f {gate} ]; do sleep 0.05; done; '
                 'else cp "$load/minute" "$run/loaded-minute"; fi\ntouch "$run/DONE"')
        self.binary.write_text(script)
        required = self.root / "required.toml"
        required.write_text('[required]\nanimals = []\nplants = ["fixture"]\n')
        job = self.submit(dict(kind="batch", hosts=["far", "fake"], parallel=1, mem_gb=1, bin=str(self.binary),
                               args=["--arm", "base=", "--config", str(self.trees["main"] / "config/input.toml"),
                                     "--required", str(required), "--hours", "0.4", "--seeds", "1"]))
        # Start on far, then force a loss after its checkpoint is committed.
        daemon.host("fake").up = False
        test_lab.wait_for(lambda: self.events("started", job), step=daemon.step)
        row = daemon.store.one("SELECT * FROM tasks WHERE job=?", job)
        remote = Path(daemon.remote_out(daemon.host("far"), daemon.job(job), daemon.job(job).tasks["batch"]))
        test_lab.wait_for(lambda: (remote / "batch/base/s1/run/ckpt/m000020/minute").is_file())
        daemon.launchers["far"].stop(row["unit"], daemon.lab_dir(row))
        test_lab.wait_for(lambda: (Path(daemon.lab_dir(row)) / "rc.json").is_file())
        daemon.task_lost(row, "simulated host loss")
        self.assertFalse(remote.exists())
        daemon.host("far").enabled = False
        daemon.host("fake").up = True
        self.assertEqual(self.finished(daemon, job)[0], "done")
        view = daemon.job(job).tasks["batch"]
        self.assertEqual(view.host, "fake")
        self.assertEqual((view.out / "batch/base/s1/run/loaded-minute").read_text(), "20\n")
        manifest = json.loads((view.out / "batch/batch.json").read_text())
        self.assertNotIn("/far/", manifest["identity"]["config"]["path"])
        self.assertEqual(len(self.events("requeued", job)), 1)


if __name__ == "__main__":
    unittest.main()
