"""Graviton simulation jobs: mocked AWS, cargo and census; localhost transport only."""

import json
import math
import os
from pathlib import Path
import shlex
import sys
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import test_lab
from labkit.hosts import allowed


class Ec2SimulationTests(test_lab.LabCase):
    def setUp(self):
        super().setUp()
        self.trees, self.binary = test_lab.test_ab.make_fixture(self.root)
        test_lab.ABKindTests.commit_change(self)
        self.log = self.root / "aws.log"
        self.script = self.root / "aws-graviton.sh"
        self.script.write_text(f'''#!/bin/sh
echo "$@" >> {self.log}
case "$1" in
  up) touch {self.root}/instance; echo i-0abc123 ;;
  status) [ ! -f {self.root}/instance ] || echo "1 i-0abc123 127.0.0.1 running spot now c7g.16xlarge" ;;
  down) rm -f {self.root}/instance ;;
esac
exit 0
''')
        self.script.chmod(0o755)
        self.data = self.inventory(ec2=dict(script=str(self.script), types=["c7g.16xlarge"], transport="local",
                                           root=str(self.root / "ec2root"), ready_file="", launcher="setsid",
                                           python=sys.executable, status_s=0, max_mem_gb=8, setup_h=0.1,
                                           instance_types={"c7g.16xlarge": dict(arch="aarch64", cpus=8,
                                                                                mem_gb=16, spot_usd_h=0.75)}))
        self.data["hosts"]["fake"].update(groups="live")
        del self.data["hosts"]["fake"]["cpus"]
        self.arm_binary = self.root / "arm-census"
        # ARM stand-in must never be executed by the desktop preflight.
        self.arm_binary.write_text(self.binary.read_text().replace("#!/bin/sh\n", '''#!/bin/sh
for arg in "$@"; do
  case "$arg" in expand=*|params=*) echo ARM-preflight >&2; exit 99 ;; esac
done
''', 1))
        self.arm_binary.chmod(0o755)
        self.required = self.root / "required.toml"
        self.required.write_text('[required]\nanimals = []\nplants = ["fixture"]\n')
        bindir = self.root / "mock-bin"
        bindir.mkdir()
        cargo = bindir / "cargo"
        cargo.write_text(f'''#!/bin/sh
echo "mock cargo $*"
cat config/input.toml
mkdir -p "$CARGO_TARGET_DIR/release/examples"
case "$PWD" in
  {self.root}/ec2root/*) source={shlex.quote(str(self.arm_binary))} ;;
  *) source={shlex.quote(str(self.binary))} ;;
esac
cp "$source" "$CARGO_TARGET_DIR/release/examples/voxel_census"
''')
        cargo.chmod(0o755)
        env = patch.dict(os.environ, {"PATH": str(bindir) + os.pathsep + os.environ["PATH"]})
        env.start()
        self.addCleanup(env.stop)

    def ab_spec(self, **extra):
        spec = test_lab.ABKindTests.spec(self, preflight_only=False, hosts=["ec2"], expect=["fauna.rate"],
                                       expect_metric=["count"], hours=0.4, est_h=0.6)
        for arm in spec["arms"].values():
            del arm["bin"]
        spec.update(extra)
        return spec

    def approval(self, daemon):
        return test_lab.wait_for(lambda: self.events("approval-needed"), step=daemon.step)[-1]

    def batch_spec(self, **extra):
        spec = dict(kind="batch", hosts=["ec2"], parallel=1, mem_gb=1, est_h=0.4,
                    binary=dict(tree=str(self.trees["main"]), commit="HEAD"), _cwd=str(self.trees["main"]),
                    args=["--arm", "base=", "--config", "config/input.toml", "--required", str(self.required),
                          "--seeds", "1", "--hours", "0.1"])
        spec.update(extra)
        return spec

    def approve_mock(self, daemon, event):
        self.assertEqual(daemon.ec2.cfg["script"], str(self.script))
        daemon.ec2.approve(event["approval"], max_hours=2, max_usd=5)

    def builds(self, daemon, arch):
        return [daemon.job(row["id"]) for row in daemon.store.q("SELECT id FROM jobs WHERE kind='build'")
                if daemon.job(row["id"]).spec["arch"] == arch]

    def size_inventory(self, cpus=(8, 16, 32)):
        cfg = self.data["ec2"]
        cfg.update(types=[f"size{n}" for n in cpus], max_mem_gb=128,
                   max_cpu_fraction=1.0, reserve_cpus=0)
        cfg["instance_types"] = {
            f"size{n}": dict(arch="aarch64", cpus=n, mem_gb=128,
                              speed=1.0, spot_usd_h=n / 32) for n in cpus}
        return cfg

    def size_request(self, count=8, threads=2, est_h=2, mem_gb=1):
        daemon = self.daemon(self.data, clock=lambda: self.now[0])
        for _ in range(count):
            self.submit(dict(kind="command", hosts=["ec2"], arch="aarch64", argv=["true"],
                             threads=threads, est_h=est_h, mem_gb=mem_gb))
        daemon.step()
        self.assertEqual(self.log.read_text() if self.log.exists() else "", "")
        return daemon, self.events("approval-needed")[0]

    def test_size_choice_fills_sixteen_cpus(self):
        self.size_inventory()
        daemon, event = self.size_request()
        self.assertEqual(event["instance_type"], "size16")
        self.assertEqual([o["type"] for o in event["options"]], ["size8", "size16", "size32"])
        self.assertAlmostEqual(event["ec2_eta_h"], 2 + daemon.ec2.cfg["setup_h"])

    def test_work_bound_queue_chooses_fastest_within_tolerance(self):
        self.size_inventory()
        _, event = self.size_request(count=128)
        costs = [o["est_usd"] for o in event["options"]]
        self.assertLessEqual(max(costs), min(costs) * 1.10)
        self.assertEqual(event["instance_type"], "size32")

    def test_memory_pressure_limits_schedule_concurrency(self):
        cfg = self.size_inventory((16,))
        cfg["types"] = ["tight", "roomy"]
        cfg["instance_types"] = {
            name: dict(arch="aarch64", cpus=16, mem_gb=memory, spot_usd_h=0.5)
            for name, memory in [("tight", 8), ("roomy", 32)]}
        count, threads, wall_h, mem_gb = 8, 2, 2, 3
        daemon, event = self.size_request(count, threads, wall_h, mem_gb)
        options = {o["type"]: o for o in event["options"]}
        self.assertEqual(event["instance_type"], "roomy")
        for name, host in zip(cfg["types"], daemon.ec2.potential_hosts()):
            parallel = min(host.usable // threads, int(host.max_mem_gb // mem_gb))
            option = options[name]
            self.assertAlmostEqual(option["eta_h"], math.ceil(count / parallel) * wall_h / host.speed + cfg["setup_h"])
            self.assertTrue(all(s["fraction"] <= parallel * threads / host.usable for s in option["fill_profile"]))

    def test_each_size_and_chosen_approval_include_fill_and_idle_cost(self):
        self.size_inventory()
        daemon, event = self.size_request()
        choice = next(o for o in event["options"] if o["type"] == event["instance_type"])
        for key in ("fill_profile", "fill_mean", "idle_usd"):
            self.assertEqual(event[key], choice[key])
        for option, host in zip(event["options"], daemon.ec2.potential_hosts()):
            busy = sum(s["fraction"] * s["hours"] * host.usable for s in option["fill_profile"])
            tasks = daemon.ec2.eligible()
            self.assertAlmostEqual(busy, sum(t.threads * t.est_h / host.speed for t in tasks))
            self.assertLessEqual(option["fill_mean"], 1)
            self.assertAlmostEqual(option["idle_usd"], option["eta_h"] * host.price_usd_h * (1 - option["fill_mean"]))

    def test_fallback_launch_records_actual_type_capacity_and_cost(self):
        cfg = self.size_inventory()
        cfg["instance_types"]["size16"]["fallback"] = "fallback16"
        cfg["instance_types"]["fallback16"] = dict(arch="aarch64", cpus=12,
                                                    mem_gb=24, speed=0.9, spot_usd_h=0.9)
        self.script.write_text(self.script.read_text().replace(
            "up) touch", "up) echo 'launched fallback16; fixture' >&2; touch"))
        daemon, event = self.size_request()
        self.assertEqual(event["fallback"], "fallback16")
        self.assertNotIn("fallback16", [o["type"] for o in event["options"]])
        self.approve_mock(daemon, event)
        self.assertIn("-t size16,fallback16", self.log.read_text())
        self.assertEqual(daemon.ec2.s["type"], "fallback16")
        self.assertEqual(daemon.store.kv_get("ec2")["type"], "fallback16")
        self.assertEqual(self.events("ec2-launching")[-1]["instance_type"], "fallback16")
        self.now[0] += 3600
        self.assertEqual(daemon.ec2.cost(), (1, 0.9))
        host = daemon.ec2.make_host("127.0.0.1")
        self.assertEqual((host.cpus, host.max_mem_gb, host.price_usd_h), (12, 24, 0.9))

    def test_type_override_uses_its_own_fallback(self):
        cfg = self.size_inventory()
        cfg["instance_types"]["size16"]["fallback"] = "fallback16"
        cfg["instance_types"]["size8"]["fallback"] = "fallback8"
        daemon, event = self.size_request()
        self.assertEqual(event["instance_type"], "size16")
        daemon.ec2.approve(event["approval"], max_hours=2, max_usd=5, instance_type="size8")
        self.assertIn("-t size8,fallback8", self.log.read_text())
        # The existing mock has no launched-type report: retain the override.
        self.assertEqual(daemon.ec2.s["type"], "size8")

    def test_candidate_must_fit_each_task(self):
        self.size_inventory()
        _, event = self.size_request(count=1, threads=12)
        self.assertEqual(event["instance_type"], "size16")
        self.assertEqual([o["type"] for o in event["options"]], ["size16", "size32"])

    def test_cost_tolerance_is_configurable(self):
        cfg = self.size_inventory()
        cfg["cost_tolerance"] = 0
        _, event = self.size_request(count=128)
        self.assertEqual(event["instance_type"], "size8")

    def test_equal_eta_and_cost_choose_fewest_cpus(self):
        cfg = self.size_inventory((32, 8))
        for info in cfg["instance_types"].values():
            info["spot_usd_h"] = 0.5
        _, event = self.size_request(count=1)
        self.assertEqual(event["instance_type"], "size8")

    def test_required_builds_fit_larger_candidate(self):
        self.size_inventory((1, 16))
        daemon = self.daemon(self.data)
        self.submit(self.ab_spec())
        daemon.step()
        event = self.events("approval-needed")[0]
        self.assertEqual(event["instance_type"], "size16")
        self.assertEqual([o["type"] for o in event["options"]], ["size16"])
        self.assertAlmostEqual(event["ec2_eta_h"], 0.6 + 2 * 0.25 + daemon.ec2.cfg["setup_h"])

    def test_ec2_ab_plans_and_estimate_includes_both_uncached_builds(self):
        daemon = self.daemon(self.data)
        job = self.submit(self.ab_spec())
        # First tick opens the request before either provider has a planned task.
        daemon.step()
        event = self.events("approval-needed")[0]
        views = daemon.job(job).tasks.values()
        self.assertEqual(self.state(daemon, job)[0], "inputs")
        self.assertTrue(all(v.task.arch == "aarch64" and v.task.hosts == ["ec2"] for v in views))
        self.assertEqual(self.log.read_text() if self.log.exists() else "", "")
        self.assertAlmostEqual(event["ec2_eta_h"], 0.6 + 2 * 0.25 + 0.1)
        self.assertEqual(event["est_usd"], round(event["ec2_eta_h"] * 0.75, 2))
        # Expanding build jobs must not count their inputs twice.
        daemon.plan_jobs()
        _, details = daemon.ec2.worth(daemon.ec2.eligible())
        self.assertAlmostEqual(details["ec2_eta_h"], event["ec2_eta_h"])

    def test_ec2_ab_builds_native_then_fetches_early_and_continuation_outputs(self):
        daemon = self.daemon(self.data)
        job_id = self.submit(self.ab_spec())
        self.approve_mock(daemon, self.approval(daemon))
        state, reason = self.finished(daemon, job_id)
        self.assertEqual(state, "done", reason)
        job = daemon.job(job_id)
        builds = self.builds(daemon, "aarch64")
        self.assertEqual(len(builds), 2)
        # The drained host went down and forgot its uploads: the next instance starts empty.
        self.assertEqual(daemon.store.q("SELECT key FROM kv WHERE key LIKE 'uploaded:ec2:%'"), [])
        self.assertNotIn("ec2", daemon.tools)
        for build in builds:
            view = build.tasks["build"]
            self.assertEqual(view.host, "ec2")
            self.assertEqual(view.task.arch, "aarch64")
            self.assertEqual(view.task.inputs, [f"source:{build.spec['commit']}"])
            self.assertIn("--source-tar", view.task.argv)
            # The same rustc as the local build: a newer one's deprecations fail `warnings = "deny"`.
            from labkit.kinds.build import local_toolchain
            self.assertIn(f"export RUSTUP_TOOLCHAIN={local_toolchain()}", view.task.argv[2])
            self.assertTrue(daemon.ctx.artifact(view.task.meta["key"]).is_file())
            log = (view.out / "stdout.log").read_text()
            self.assertIn("mock cargo", log)
            self.assertIn((Path(build.spec["repo"]) / "config/input.toml").read_text(), log)
        sim_events = [e["seq"] for e in self.events("started", job_id)]
        build_events = [e["seq"] for e in self.events("task-done") if e.get("job") in {b.id for b in builds}]
        self.assertLess(max(build_events), min(sim_events))
        self.assertEqual({v.task.meta["stage"] for v in job.tasks.values()}, {"early", "continuation"})
        for view in job.tasks.values():
            self.assertEqual(view.host, "ec2")
            self.assertEqual(view.task.arch, "aarch64")
            self.assertIn(":aarch64:voxel_census", view.task.inputs[0])
            self.assertTrue((view.out / "census.csv").is_file())
            self.assertTrue((view.out / "run/ckpt/m000020/minute").is_file())
            launch = json.loads((job.out / ".lab" / view.name / "launch.json").read_text())
            self.assertIn("/ec2root/artifacts/", launch["argv"][0])
        self.assertTrue((job.out / "preflight.log").is_file())
        # Cached per commit/arch: planning another job creates no build providers.
        again = self.submit(self.ab_spec())
        daemon.ingest()
        daemon.plan_jobs()
        daemon.resolve_inputs()
        self.assertEqual(self.state(daemon, again)[0], "preflight")
        self.assertEqual(len(self.builds(daemon, "aarch64")), 2)
        _, details = daemon.ec2.worth([v.task for v in daemon.job(again).tasks.values()])
        self.assertAlmostEqual(details["ec2_eta_h"], 0.6 + 0.1)

    def test_ab_without_ec2_keeps_x86_keys_and_local_placement(self):
        daemon = self.daemon(self.data)
        job_id = self.submit(self.ab_spec(hosts=None))
        state, reason = self.finished(daemon, job_id)
        self.assertEqual(state, "done", reason)
        for view in daemon.job(job_id).tasks.values():
            self.assertEqual(view.host, "fake")
            self.assertEqual(view.task.arch, "x86_64")
            self.assertIn(":x86_64:voxel_census", view.task.inputs[0])
            self.assertFalse(allowed(view.task, daemon.ec2.potential_hosts()[0]))
        self.assertEqual(self.builds(daemon, "aarch64"), [])
        self.assertEqual(self.events("approval-needed"), [])

    def test_mixed_hosts_keep_all_ab_arms_on_ec2_arch(self):
        daemon = self.daemon(self.data)
        job_id = self.submit(self.ab_spec(hosts=["ec2", "fake", "eidolon"], seeds="1-2"))
        self.approval(daemon)
        job = daemon.job(job_id)
        self.assertEqual(job.state["arch"], "aarch64")
        self.assertEqual(len(job.tasks), 4)
        for view in job.tasks.values():
            self.assertTrue(allowed(view.task, daemon.ec2.potential_hosts()[0]))
            self.assertFalse(allowed(view.task, daemon.inv.hosts["fake"]))

    def test_ec2_batch_builds_native_and_fetches_outputs(self):
        daemon = self.daemon(self.data)
        job_id = self.submit(self.batch_spec())
        event = self.approval(daemon)
        self.assertAlmostEqual(event["ec2_eta_h"], 0.4 + 0.25 + 0.1)
        self.approve_mock(daemon, event)
        state, reason = self.finished(daemon, job_id)
        self.assertEqual(state, "done", reason)
        view = daemon.job(job_id).tasks["batch"]
        self.assertEqual(view.host, "ec2")
        self.assertEqual(view.task.arch, "aarch64")
        self.assertIn(":aarch64:voxel_census", view.task.inputs[0])
        self.assertIn("\tdone\t", (view.out / "batch/STATE.tsv").read_text())
        self.assertTrue((view.out / "batch/base/s1/census.csv").is_file())
        self.assertEqual(len(self.builds(daemon, "aarch64")), 1)

    def test_supplied_x86_arm_binaries_stay_local_and_arm_sims_build_from_source(self):
        daemon = self.daemon(self.data)
        spec = self.ab_spec()
        for arm in spec["arms"].values():
            arm["bin"] = str(self.binary)
        job_id = self.submit(spec)
        self.approve_mock(daemon, self.approval(daemon))
        state, reason = self.finished(daemon, job_id)
        self.assertEqual(state, "done", reason)
        self.assertEqual(len(self.builds(daemon, "aarch64")), 2)
        self.assertEqual(self.builds(daemon, "x86_64"), [])
        for view in daemon.job(job_id).tasks.values():
            self.assertNotIn(str(self.binary), view.task.files)
            self.assertIn(":aarch64:voxel_census", view.task.inputs[0])

    def test_ec2_preflight_only_needs_no_arm_build_or_approval(self):
        daemon = self.daemon(self.data)
        job_id = self.submit(self.ab_spec(preflight_only=True))
        state, reason = self.finished(daemon, job_id)
        self.assertEqual(state, "done", reason)
        self.assertEqual(daemon.job(job_id).tasks, {})
        self.assertEqual(self.builds(daemon, "aarch64"), [])
        self.assertEqual(self.events("approval-needed"), [])

    def test_batch_without_ec2_keeps_x86_binary_and_local_placement(self):
        daemon = self.daemon(self.data)
        job_id = self.submit(self.batch_spec(hosts=None))
        state, reason = self.finished(daemon, job_id)
        self.assertEqual(state, "done", reason)
        view = daemon.job(job_id).tasks["batch"]
        self.assertEqual(view.host, "fake")
        self.assertIn(":x86_64:voxel_census", view.task.inputs[0])
        self.assertEqual(self.events("approval-needed"), [])

    def test_ec2_batch_bare_bin_requires_source_commit(self):
        daemon = self.daemon(self.data)
        spec = self.batch_spec(bin=str(self.binary))
        del spec["binary"]
        job_id = self.submit(spec)
        state, reason = self.finished(daemon, job_id)
        self.assertEqual(state, "aborted")
        self.assertIn("bin has no source commit", reason)
        self.assertEqual(self.events("approval-needed"), [])

    def test_restored_ec2_host_probes_its_l3_groups(self):
        daemon = self.daemon(self.data)
        self.submit(self.ab_spec())
        self.approve_mock(daemon, self.approval(daemon))
        daemon.ec2.tick()
        self.assertEqual(daemon.ec2.s["state"], "up")
        self.assertTrue(daemon.ec2.host.groups)
        restored = self.daemon(self.data)
        self.assertEqual(restored.ec2.s["state"], "up")
        self.assertTrue(restored.ec2.host.groups)
        self.assertEqual(restored.ec2.host.arch, "aarch64")
