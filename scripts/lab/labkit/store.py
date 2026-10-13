"""State: paths, the sqlite store, the event feed and the submission spool."""

from dataclasses import dataclass
import datetime
import json
import os
from pathlib import Path
import secrets
import sqlite3
import threading
import time

REPO = Path(__file__).resolve().parents[3]
TERMINAL = ("done", "aborted", "cancelled", "failed", "rejected")


def refuse_tmpfs(path):
    """Never keep state, trees or targets in RAM: an OOM in tmpfs killed a session."""
    path = Path(path).expanduser().absolute()
    best, kind = "", ""
    try:
        for line in Path("/proc/mounts").read_text().splitlines():
            parts = line.split()
            mount = parts[1].replace("\\040", " ")
            inside = str(path) == mount or str(path).startswith(mount.rstrip("/") + "/")
            if inside and len(mount) >= len(best):
                best, kind = mount, parts[2]
    except OSError:
        return path
    if kind in ("tmpfs", "ramfs") or str(path).startswith("/tmp/"):
        raise ValueError(f"{path} is on {kind or '/tmp'} (RAM); labd refuses state, trees and targets there")
    return path


@dataclass
class Paths:
    state: Path
    cache: Path
    runs: Path
    hosts: Path
    repo: Path = REPO

    @classmethod
    def from_env(cls, state=None, cache=None, runs=None, hosts=None):
        env = os.environ.get
        return cls(state=Path(state or env("LABD_STATE") or "~/.local/state/labd").expanduser(),
                   cache=Path(cache or env("LABD_CACHE") or "~/.cache/labd").expanduser(),
                   runs=Path(runs or env("LABD_RUNS") or REPO / "runs/lab").expanduser(),
                   hosts=Path(hosts or env("LABD_HOSTS") or REPO / "config/lab/hosts.toml").expanduser())

    def ensure(self):
        for path in (self.state, self.cache, self.runs):
            refuse_tmpfs(path)
            path.mkdir(parents=True, exist_ok=True)
        for sub in ("incoming", "control", "rejected"):
            (self.state / sub).mkdir(exist_ok=True)
        return self

    @property
    def db(self):
        return self.state / "labd.sqlite"

    @property
    def events(self):
        return self.state / "events.jsonl"


def atomic_write(path, text):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.{secrets.token_hex(3)}.tmp")
    temporary.write_text(text)
    temporary.replace(path)


def new_id(kind, name=None):
    stamp = datetime.datetime.now().strftime("%m%d-%H%M%S")
    label = "".join(c if c.isalnum() or c in "-_" else "-" for c in (name or kind))[:24]
    return f"{stamp}-{label}-{secrets.token_hex(2)}"


def spool(paths, directory, payload, stem):
    paths.ensure()
    atomic_write(paths.state / directory / f"{stem}.json", json.dumps(payload, indent=1))
    return paths.state / directory / f"{stem}.json"


SCHEMA = """
CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY, kind TEXT, name TEXT, state TEXT, spec TEXT,
  kstate TEXT DEFAULT '{}', out TEXT, priority INTEGER DEFAULT 0, submitted REAL, started REAL,
  finished REAL, reason TEXT, parent TEXT, last_progress REAL DEFAULT 0, summary TEXT);
CREATE TABLE IF NOT EXISTS tasks(job TEXT, name TEXT, seq INTEGER, state TEXT, spec TEXT, host TEXT,
  grp INTEGER, unit TEXT, launcher TEXT, started REAL, finished REAL, rc INTEGER, reason TEXT,
  attempts INTEGER DEFAULT 0, live TEXT DEFAULT '{}', PRIMARY KEY(job, name));
CREATE TABLE IF NOT EXISTS artifacts(key TEXT PRIMARY KEY, path TEXT, sha256 TEXT, meta TEXT, created REAL);
CREATE TABLE IF NOT EXISTS recipes(key TEXT PRIMARY KEY, spec TEXT);
CREATE TABLE IF NOT EXISTS providers(key TEXT PRIMARY KEY, job TEXT);
CREATE TABLE IF NOT EXISTS approvals(id TEXT PRIMARY KEY, kind TEXT, state TEXT, request TEXT,
  grant_ TEXT, created REAL, decided REAL);
CREATE TABLE IF NOT EXISTS kv(key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE IF NOT EXISTS ec2_grants(id INTEGER PRIMARY KEY AUTOINCREMENT, at REAL, usd REAL, note TEXT);
CREATE TABLE IF NOT EXISTS ec2_spend(key TEXT PRIMARY KEY, instance TEXT, approval TEXT, ended REAL,
  hours REAL, usd REAL, reason TEXT);
"""


class Store:
    """One sqlite file, WAL, serialised by a lock so preflight threads can read it too."""

    def __init__(self, paths, readonly=False):
        self.paths = paths
        self.lock = threading.RLock()
        if readonly:
            if not paths.db.exists():
                self.db = sqlite3.connect(":memory:", check_same_thread=False)
                self.db.executescript(SCHEMA)
            else:
                self.db = sqlite3.connect(f"file:{paths.db}?mode=ro", uri=True, check_same_thread=False)
        else:
            paths.ensure()
            self.db = sqlite3.connect(paths.db, check_same_thread=False, isolation_level=None)
            self.db.execute("PRAGMA journal_mode=WAL")
            self.db.executescript(SCHEMA)
        self.db.row_factory = sqlite3.Row

    def q(self, sql, *args):
        with self.lock:
            return self.db.execute(sql, args).fetchall()

    def one(self, sql, *args):
        rows = self.q(sql, *args)
        return rows[0] if rows else None

    def x(self, sql, *args):
        with self.lock:
            self.db.execute(sql, args)

    def kv_get(self, key, default=None):
        row = self.one("SELECT value FROM kv WHERE key=?", key)
        return json.loads(row["value"]) if row else default

    def kv_set(self, key, value):
        self.x("INSERT INTO kv(key, value) VALUES(?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
               key, json.dumps(value))

    def update(self, table, where, **fields):
        cols = ", ".join(f"{k}=?" for k in fields)
        keys = " AND ".join(f"{k}=?" for k in where)
        values = [json.dumps(v) if isinstance(v, (dict, list)) else v for v in fields.values()]
        self.x(f"UPDATE {table} SET {cols} WHERE {keys}", *values, *where.values())


class Events:
    """Append-only events.jsonl; the daemon is the only writer."""

    def __init__(self, paths, store):
        self.path = paths.events
        self.store = store
        self.seq = store.kv_get("event_seq", 0)

    def emit(self, event, **fields):
        with self.store.lock:
            self.seq += 1
            now = time.time()
            record = dict(seq=self.seq, ts=datetime.datetime.fromtimestamp(now).isoformat(timespec="seconds"),
                          t=round(now, 3), event=event)
            record.update({k: v for k, v in fields.items() if v is not None})
            with self.path.open("a") as stream:
                stream.write(json.dumps(record, default=str) + "\n")
            self.store.kv_set("event_seq", self.seq)
            return record


def read_events(path, offset=0):
    """Complete lines from byte offset; returns (records, new offset)."""
    try:
        with Path(path).open("rb") as stream:
            stream.seek(offset)
            data = stream.read()
    except FileNotFoundError:
        return [], offset
    end = data.rfind(b"\n") + 1
    records = []
    for line in data[:end].splitlines():
        try:
            records.append(json.loads(line))
        except ValueError:
            pass
    return records, offset + end
