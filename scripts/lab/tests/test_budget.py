"""EC2 budget laws: grants, spend, commitments and refusals. Mocked AWS; arbitrary numbers."""

import contextlib
import io
import json
from pathlib import Path
import sqlite3
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import test_lab
from labkit import budget, cli

OLD_SCHEMA = """
CREATE TABLE jobs(id TEXT PRIMARY KEY, kind TEXT, name TEXT, state TEXT, spec TEXT,
  kstate TEXT DEFAULT '{}', out TEXT, priority INTEGER DEFAULT 0, submitted REAL, started REAL,
  finished REAL, reason TEXT, parent TEXT, last_progress REAL DEFAULT 0, summary TEXT);
CREATE TABLE tasks(job TEXT, name TEXT, seq INTEGER, state TEXT, spec TEXT, host TEXT,
  grp INTEGER, unit TEXT, launcher TEXT, started REAL, finished REAL, rc INTEGER, reason TEXT,
  attempts INTEGER DEFAULT 0, live TEXT DEFAULT '{}', PRIMARY KEY(job, name));
CREATE TABLE artifacts(key TEXT PRIMARY KEY, path TEXT, sha256 TEXT, meta TEXT, created REAL);
CREATE TABLE recipes(key TEXT PRIMARY KEY, spec TEXT);
CREATE TABLE providers(key TEXT PRIMARY KEY, job TEXT);
CREATE TABLE approvals(id TEXT PRIMARY KEY, kind TEXT, state TEXT, request TEXT,
  grant_ TEXT, created REAL, decided REAL);
CREATE TABLE kv(key TEXT PRIMARY KEY, value TEXT);
"""


class BudgetTests(test_lab.LabCase):
    price = 0.6

    def setUp(self):
        super().setUp()
        self.log = self.root / "aws.log"
        script = self.root / "aws-graviton.sh"
        # The instance never reports running: it stays `launching`, which already costs money.
        script.write_text(f'''#!/bin/sh
echo "$@" >> {self.log}
case "$1" in
  up) touch {self.root}/instance; echo i-0abc123 ;;
  status) [ ! -f {self.root}/instance ] || echo "1 i-0abc123 None pending spot now t1" ;;
  down) rm -f {self.root}/instance ;;
esac
exit 0
''')
        script.chmod(0o755)
        self.data = self.inventory(ec2=dict(script=str(script), types=["t1"], transport="local", ready_file="",
                                           status_s=0, launch_timeout_s=1e12, setup_h=0.1,
                                           instance_types={"t1": dict(arch="aarch64", cpus=8, mem_gb=16,
                                                                      spot_usd_h=self.price)}))

    def daemon_at(self):
        return self.daemon(self.data, clock=lambda: self.now[0])

    def request(self, daemon):
        self.submit(dict(kind="command", hosts=["ec2"], arch="aarch64", argv=["true"], threads=2, est_h=1,
                         mem_gb=1))
        test_lab.wait_for(lambda: self.events("approval-needed"), step=daemon.step)
        return self.events("approval-needed")[-1]["approval"]

    def ups(self):
        return [line for line in (self.log.read_text().splitlines() if self.log.exists() else [])
                if line.startswith("up")]

    def remaining(self, daemon):
        return daemon.ec2.budget()["remaining"]

    def cli(self, *argv):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            rc = cli.main(["--state", str(self.paths.state), "--hosts", str(self.root / "missing.toml"), *argv])
        return rc, out.getvalue(), err.getvalue()

    # 1 ---------------------------------------------------------------------------------
    def test_approval_within_budget_launches_and_one_above_is_refused(self):
        daemon = self.daemon_at()
        budget.grant(daemon.store, 10, "test grant", self.now[0])
        request = self.request(daemon)
        with self.assertRaises(budget.BudgetError) as refused:
            daemon.ec2.approve(request, max_hours=100, max_usd=10.01)
        self.assertIn("10.00 left", str(refused.exception))
        self.assertIn("Wrysk", str(refused.exception))
        self.assertEqual(self.ups(), [])
        self.assertEqual(daemon.store.one("SELECT state FROM approvals WHERE id=?", request)["state"], "open")
        self.assertEqual(daemon.ec2.s["state"], "approval-needed")
        daemon.ec2.approve(request, max_hours=100, max_usd=10)
        self.assertEqual(len(self.ups()), 1)
        self.assertEqual(daemon.ec2.s["state"], "launching")

    def test_cli_and_control_refuse_over_budget_before_launch(self):
        daemon = self.daemon_at()
        budget.grant(daemon.store, 3, "test grant", self.now[0])
        request = self.request(daemon)
        rc, _, err = self.cli("approve", "ec2", request, "--max-hours", "1", "--max-usd", "4")
        self.assertEqual(rc, 1)
        self.assertIn("3.00 left", err)
        self.assertEqual(list((self.paths.state / "control").glob("*.json")), [])
        # A control file that bypassed the CLI check is still refused by the daemon.
        test_lab.spool(self.paths, "control", dict(op="approve", what="ec2", id=request, max_hours=1, max_usd=4),
                       "approve")
        daemon.step()
        self.assertEqual(self.ups(), [])
        self.assertTrue(self.events("approval-refused"))

    # 2 ---------------------------------------------------------------------------------
    def test_finished_spend_lowers_remaining_and_survives_restart(self):
        daemon = self.daemon_at()
        budget.grant(daemon.store, 10, "test grant", self.now[0])
        daemon.ec2.approve(self.request(daemon), max_hours=100, max_usd=5)
        hours = 2.5
        self.now[0] += hours * 3600
        self.assertTrue(daemon.ec2.down("test"))
        expected = 10 - hours * self.price
        self.assertAlmostEqual(self.remaining(daemon), expected, places=6)
        self.assertAlmostEqual(daemon.ec2.budget()["committed"], 0)
        restarted = self.daemon_at()
        self.assertAlmostEqual(self.remaining(restarted), expected, places=6)
        self.assertEqual(len(restarted.ec2.budget()["finished"]), 1)

    def test_spot_loss_spend_is_counted(self):
        daemon = self.daemon_at()
        budget.grant(daemon.store, 10, "test grant", self.now[0])
        daemon.ec2.approve(self.request(daemon), max_hours=100, max_usd=5)
        daemon.ec2.s["state"] = "up"
        (self.root / "instance").unlink()
        self.now[0] += 3600
        daemon.ec2.tick()
        self.assertEqual(daemon.ec2.s["state"], "idle")
        self.assertAlmostEqual(self.remaining(daemon), 10 - self.price, places=6)

    # 3 ---------------------------------------------------------------------------------
    def test_running_approval_is_committed_and_its_dollars_cannot_be_reused(self):
        daemon = self.daemon_at()
        budget.grant(daemon.store, 10, "test grant", self.now[0])
        first = self.request(daemon)
        daemon.ec2.approve(first, max_hours=100, max_usd=6)
        self.now[0] += 3600
        state = daemon.ec2.budget()
        self.assertAlmostEqual(state["spent"], self.price, places=6)
        self.assertAlmostEqual(state["committed"], 6 - self.price, places=6)
        self.assertAlmostEqual(state["remaining"], 4, places=6)
        # A second request that fits only if the first's commitment were ignored.
        daemon.store.x("INSERT INTO approvals(id, kind, state, request, created) VALUES('ec2-second', 'ec2', "
                       "'open', ?, ?)", json.dumps(dict(instance_type="t1", jobs=[])), self.now[0])
        self.assertLess(5, 10 - state["spent"])
        with self.assertRaises(budget.BudgetError):
            daemon.ec2.approve("ec2-second", max_hours=100, max_usd=5)
        self.assertEqual(len(self.ups()), 1)
        self.assertEqual(daemon.ec2.s["approval"], first)
        self.assertEqual(daemon.ec2.s["state"], "launching")

    def test_over_budget_running_instance_is_never_stopped_by_the_budget(self):
        daemon = self.daemon_at()
        budget.grant(daemon.store, 1, "test grant", self.now[0])
        daemon.ec2.approve(self.request(daemon), max_hours=100, max_usd=1)
        daemon.store.x("DELETE FROM ec2_grants")  # budget now negative
        self.now[0] += 60
        daemon.step()
        self.assertEqual(daemon.ec2.s["state"], "launching")
        self.assertNotIn("down", self.log.read_text())

    # 4 ---------------------------------------------------------------------------------
    def test_grant_raises_remaining_and_note_is_shown_and_required(self):
        daemon = self.daemon_at()
        before = self.remaining(daemon)
        rc, out, _ = self.cli("budget", "grant", "7.5", "--note", "go ahead, seven fifty")
        self.assertEqual(rc, 0)
        daemon.step()
        self.assertAlmostEqual(self.remaining(daemon) - before, 7.5)
        rc, out, _ = self.cli("budget")
        self.assertEqual(rc, 0)
        self.assertIn("go ahead, seven fifty", out)
        rc, _, err = self.cli("budget", "grant", "5")
        self.assertEqual(rc, 1)
        self.assertIn("note", err)
        rc, _, _ = self.cli("budget", "grant", "5", "--note", "  ")
        self.assertEqual(rc, 1)
        self.assertEqual(list((self.paths.state / "control").glob("*.json")), [])
        with self.assertRaises(ValueError):
            budget.grant(daemon.store, 5, "", self.now[0])
        daemon.step()
        self.assertAlmostEqual(self.remaining(daemon) - before, 7.5)

    def test_status_shows_budget_line(self):
        daemon = self.daemon_at()
        budget.grant(daemon.store, 4, "test grant", self.now[0])
        rc, out, _ = self.cli("status")
        self.assertEqual(rc, 0)
        self.assertIn("ec2 budget: $4.00 left of $4.00", out)

    # 5 ---------------------------------------------------------------------------------
    def test_migration_keeps_rows_and_counts_past_spend_once(self):
        db = sqlite3.connect(self.paths.db)
        db.executescript(OLD_SCHEMA)
        db.execute("INSERT INTO jobs(id, kind, state, spec) VALUES('j1', 'command', 'done', '{}')")
        db.execute("INSERT INTO tasks(job, name, state, spec) VALUES('j1', 't1', 'done', '{}')")
        db.execute("INSERT INTO approvals(id, kind, state, request) VALUES('ec2-old', 'ec2', 'granted', '{}')")
        db.execute("INSERT INTO kv(key, value) VALUES('event_seq', '4')")
        db.commit()
        db.close()
        spends = [1.25, 0.5]
        events = [dict(seq=1, t=self.now[0], event="host-down", host="ec2", hours=1, est_usd=spends[0]),
                  dict(seq=2, t=self.now[0], event="host-down", host="eidolon"),
                  dict(seq=3, t=self.now[0], event="host-down", host="ec2", reason="spot loss"),
                  dict(seq=4, t=self.now[0], event="host-down", host="ec2", hours=1, est_usd=spends[1])]
        self.paths.events.write_text("".join(json.dumps(e) + "\n" for e in events))
        # Read-only status before any daemon start already counts the log's spend.
        rc, out, _ = self.cli("budget")
        self.assertIn(f"${sum(spends):.2f}", out)
        for _ in range(2):  # two starts = the migration run twice
            daemon = self.daemon_at()
            self.assertAlmostEqual(daemon.ec2.budget()["spent"], sum(spends))
            budget.migrate(daemon.store, self.paths.events)
            self.assertAlmostEqual(daemon.ec2.budget()["spent"], sum(spends))
        for table, count in (("jobs", 1), ("tasks", 1), ("approvals", 1)):
            self.assertEqual(daemon.store.one(f"SELECT COUNT(*) AS n FROM {table}")["n"], count)
        self.assertEqual(daemon.store.one("SELECT state FROM approvals WHERE id='ec2-old'")["state"], "granted")


if __name__ == "__main__":
    import unittest
    unittest.main()
