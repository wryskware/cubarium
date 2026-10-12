"""ab keeps its healthy runs when one run is lost or fails (eidolon reboot, 2026-10-11).

Real ab jobs on the fake local host, with the shared fake census binary plus two knobs:
`fail` lists run-path fragments that exit 3, and `hold` makes continuations wait for `gate`.
"""

import json
import subprocess

import test_lab
import test_ab


class ABPartialTests(test_lab.LabCase):
    def setUp(self):
        super().setUp()
        self.trees, self.binary = test_ab.make_fixture(self.root)
        self.fail, self.hold, self.gate = self.root / "fail", self.root / "hold", self.root / "gate"
        text = self.binary.read_text()
        knobs = (f'for p in $(cat {self.fail} 2>/dev/null); do case "$run" in *"$p"*) exit 3 ;; esac; done\n'
                 f'if [ -n "$load" ] && [ -f {self.hold} ]; then\n'
                 f'  while [ ! -f {self.gate} ]; do sleep 0.05; done\nfi\n')
        self.binary.write_text(text.replace('mkdir -p "$run/ckpt', knobs + 'mkdir -p "$run/ckpt', 1))
        config = self.trees["branch"] / "config/input.toml"
        config.write_text(config.read_text().replace("rate=3", "rate=5"))
        subprocess.run(["git", "-C", str(self.trees["branch"]), "-c", "user.name=T", "-c", "user.email=t@x.invalid",
                        "commit", "-qam", "change"], check=True)
        self.data = self.inventory()
        self.data["hosts"]["fake"].update(groups="live")
        del self.data["hosts"]["fake"]["cpus"]

    def start(self, seeds="1-2", fail=()):
        self.fail.write_text("\n".join(fail) + "\n")
        daemon = self.daemon(self.data)
        arms = {n: dict(tree=str(t), bin=str(self.binary), commit="HEAD") for n, t in self.trees.items()}
        job = self.submit(dict(kind="ab", arms=arms, config_path="config/input.toml", seeds=seeds,
                               expect=["fauna.rate"], expect_metric=["count"], hours=0.4, mem_gb=1))
        return daemon, job

    def tasks(self, daemon, job, stage=None):
        rows = daemon.store.q("SELECT name, state, spec FROM tasks WHERE job=?", job)
        return {r["name"]: r["state"] for r in rows if stage is None or json.loads(r["spec"])["meta"]["stage"] == stage}

    def summary(self, daemon, job):
        return daemon.store.one("SELECT summary FROM jobs WHERE id=?", job)["summary"]

    def running_continuations(self, daemon, job):
        """Hold continuations at the gate until every one of them runs."""
        test_lab.wait_for(lambda: (lambda c: len(c) == 4 and all(s == "running" for s in c.values()))(
            self.tasks(daemon, job, "continuation")), step=daemon.step)

    def assert_others_carry_on(self, daemon, job, gone):
        for _ in range(5):
            daemon.step()
        self.assertFalse(self.events("aborting", job))
        others = {n: s for n, s in self.tasks(daemon, job, "continuation").items() if n != gone}
        self.assertTrue(others and all(s == "running" for s in others.values()), others)
        self.assertEqual(self.state(daemon, job)[0], "running")

    def assert_done_naming(self, daemon, job, gone):
        self.gate.touch()
        state, reason = self.finished(daemon, job)
        self.assertEqual(state, "done", reason)
        self.assertIn(gone, reason)  # lab status / lab wait show the reason
        self.assertIn(gone, self.summary(daemon, job))
        self.assertTrue(any(line.startswith("missing: arm=branch seed=2") for line in self.summary(daemon, job).splitlines()))
        self.assertTrue(self.events("done", job))
        healthy = {n: s for n, s in self.tasks(daemon, job, "continuation").items() if n != gone}
        self.assertTrue(all(s == "done" for s in healthy.values()), healthy)

    def test_lost_continuation_keeps_the_others_and_finishes_done(self):
        self.hold.touch()
        daemon, job = self.start()
        self.running_continuations(daemon, job)
        gone = "continuation-branch-s2"
        row = daemon.store.one("SELECT * FROM tasks WHERE job=? AND name=?", job, gone)
        daemon.launchers[row["host"]].stop(row["unit"], daemon.lab_dir(row))
        # The retry is spent: the next loss is final.
        daemon.store.update("tasks", dict(job=job, name=gone), attempts=row["attempts"] + 5)
        row = daemon.store.one("SELECT * FROM tasks WHERE job=? AND name=?", job, gone)
        daemon.task_lost(row, "task-lost: unit/process gone without rc.json")
        self.assertEqual(self.tasks(daemon, job)[gone], "lost")
        self.assert_others_carry_on(daemon, job, gone)
        self.assert_done_naming(daemon, job, gone)

    def test_failed_continuation_keeps_the_others_and_finishes_done(self):
        self.hold.touch()
        gone = "continuation-branch-s2"
        daemon, job = self.start(fail=["continuation/branch/s2/"])
        test_lab.wait_for(lambda: self.tasks(daemon, job).get(gone) == "failed", step=daemon.step)
        self.running_continuations_except(daemon, job, gone)
        self.assert_others_carry_on(daemon, job, gone)
        self.assert_done_naming(daemon, job, gone)

    def running_continuations_except(self, daemon, job, gone):
        test_lab.wait_for(lambda: (lambda c: len(c) == 4 and all(s == "running" for n, s in c.items() if n != gone))(
            self.tasks(daemon, job, "continuation")), step=daemon.step)

    def test_no_seed_with_both_arms_done_aborts(self):
        daemon, job = self.start(fail=["continuation/branch/s1/", "continuation/main/s2/"])
        state, reason = self.finished(daemon, job)
        self.assertEqual(state, "aborted")
        self.assertIn("no seed has every arm done", reason)

    def test_early_loss_drops_that_seed_from_every_arms_continuation(self):
        # Built: the seed is dropped (not an abort); continuations run only for complete seeds.
        daemon, job = self.start(fail=["early/branch/s2/"])
        state, reason = self.finished(daemon, job)
        self.assertEqual(state, "done", reason)
        self.assertIn("early-branch-s2", reason)
        continuations = self.tasks(daemon, job, "continuation")
        self.assertTrue(continuations and all(n.endswith("-s1") for n in continuations), continuations)
        self.assertTrue(all(s == "done" for s in continuations.values()))
        self.assertIn("dropped seed 2", self.summary(daemon, job))
        out = daemon.store.one("SELECT out FROM jobs WHERE id=?", job)["out"]
        self.assertIn("dropped seed 2", (test_lab.Path(out) / "early.log").read_text())

    def test_early_loss_of_every_seed_aborts(self):
        daemon, job = self.start(fail=["early/branch/"])
        state, reason = self.finished(daemon, job)
        self.assertEqual(state, "aborted")
        self.assertIn("no seed has every arm done", reason)
        self.assertEqual(self.tasks(daemon, job, "continuation"), {})
