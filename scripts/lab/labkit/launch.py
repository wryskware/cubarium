"""Transports (local, ssh) and launchers (systemd transient units, setsid fallback).

Every task runs taskwrap.py. Locally that is its own transient unit:
  systemd-run --user --collect --unit=lab-<job>-<task> ... taskwrap.py launch.json
so restarting labd never kills a task and an OOM stays inside that unit. The setsid
launcher exists only for hosts where user units would not survive (eidolon has
Linger=no, so its user manager stops with the last session) and for hermetic tests.
"""

import json
import shlex
import subprocess


class LocalTransport:
    remote = False

    def run(self, script, timeout=60):
        return subprocess.run(["bash", "-s"], input=script, capture_output=True, text=True, timeout=timeout)

    def put(self, local, remote, timeout=600):
        if str(local) == str(remote):
            return
        subprocess.run(["mkdir", "-p", str(remote).rsplit("/", 1)[0]], check=True)
        subprocess.run(["cp", "-a", "--reflink=auto", str(local), str(remote)], check=True, timeout=timeout)

    def get(self, remote, local, timeout=600):
        if str(local) == str(remote):
            return
        subprocess.run(["mkdir", "-p", str(local)], check=True)
        subprocess.run(["cp", "-a", "--reflink=auto", f"{remote}/.", str(local)], check=True, timeout=timeout)

    def put_dir(self, local, remote, timeout=600):
        self.get(local, remote, timeout)

    def put_files(self, local, remote, files, timeout=600):
        """Copy the listed files (relative to local) under remote, keeping their relative paths."""
        if not files or str(local) == str(remote):
            return
        subprocess.run(["mkdir", "-p", str(remote)], check=True)
        subprocess.run(["cp", "-a", "--reflink=auto", "--parents", "--", *files, str(remote)], cwd=str(local),
                       check=True, timeout=timeout, capture_output=True)


class SshTransport:
    """ssh HOST 'bash -s' with the script on stdin: works under a fish login shell."""
    remote = True

    def __init__(self, address, options=()):
        self.address = address
        self.options = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", *options]

    def run(self, script, timeout=60):
        return subprocess.run(["ssh", *self.options, self.address, "bash -s"], input=script,
                              capture_output=True, text=True, timeout=timeout)

    def _rsh(self):
        return "ssh " + " ".join(shlex.quote(o) for o in self.options)

    def put(self, local, remote, timeout=600):
        self.run(f"mkdir -p {shlex.quote(str(remote).rsplit('/', 1)[0])}")
        subprocess.run(["rsync", "-a", "-e", self._rsh(), str(local), f"{self.address}:{remote}"],
                       check=True, timeout=timeout, capture_output=True)

    def get(self, remote, local, timeout=600):
        subprocess.run(["mkdir", "-p", str(local)], check=True)
        subprocess.run(["rsync", "-a", "-e", self._rsh(), f"{self.address}:{remote}/", f"{local}/"],
                       check=True, timeout=timeout, capture_output=True)

    def put_dir(self, local, remote, timeout=600):
        self.run(f"mkdir -p {shlex.quote(str(remote))}")
        subprocess.run(["rsync", "-a", "-e", self._rsh(), f"{local}/", f"{self.address}:{remote}/"],
                       check=True, timeout=timeout, capture_output=True)

    def put_files(self, local, remote, files, timeout=600):
        """rsync only the listed files (relative to local); a resume never sends old checkpoints."""
        if not files:
            return
        done = self.run(f"mkdir -p {shlex.quote(str(remote))}")
        if done.returncode:
            raise RuntimeError(f"mkdir {remote}: {done.stderr.strip()[-300:]}")
        subprocess.run(["rsync", "-a", "--files-from=-", "-e", self._rsh(), f"{local}/", f"{self.address}:{remote}/"],
                       input="\n".join(files) + "\n", text=True, check=True, timeout=timeout, capture_output=True)


def write_file(transport, path, text):
    if not transport.remote:
        from .store import atomic_write
        atomic_write(path, text)
        return
    q = shlex.quote(str(path))
    done = transport.run(f"mkdir -p \"$(dirname {q})\" && cat > {q}.tmp <<'LABD_EOF'\n{text}\nLABD_EOF\nmv {q}.tmp {q}\n")
    if done.returncode:
        raise RuntimeError(f"cannot write {path}: {done.stderr.strip()[-300:]}")


class LaunchError(RuntimeError):
    """A launcher's start script failed; returncode 255 is ssh itself (the host, not the task)."""

    def __init__(self, message, returncode):
        super().__init__(message)
        self.returncode = returncode


class Launcher:
    name = "?"

    def __init__(self, transport, python="python3"):
        self.transport = transport
        self.python = python

    def poll(self, entries):
        """entries: [(unit, lab_dir)] -> {unit: (rc dict or None, alive bool)}"""
        if not entries:
            return {}
        lines = ["for pair in " + " ".join(shlex.quote(f"{u}|{d}") for u, d in entries) + "; do",
                 '  u=${pair%%|*}; d=${pair#*|}',
                 '  if [ -f "$d/rc.json" ]; then echo "$u RC $(tr -d "\\n" < "$d/rc.json")"; continue; fi',
                 f"  {self.alive_check('$u', '$d')}",
                 "done"]
        done = self.transport.run("\n".join(lines) + "\n", timeout=60)
        if done.returncode:
            raise RuntimeError(f"poll failed: {done.stderr.strip()[-300:]}")
        result = {}
        for line in done.stdout.splitlines():
            unit, _, rest = line.partition(" ")
            word, _, payload = rest.partition(" ")
            if word == "RC":
                try:
                    result[unit] = (json.loads(payload), False)
                except ValueError:
                    result[unit] = (None, True)
            else:
                result[unit] = (None, word == "ALIVE")
        return result


class SystemdLauncher(Launcher):
    name = "systemd"

    def start(self, unit, launch_path, lab_dir, cwd, mem_gb=None, timeout_h=None, taskwrap=None):
        props = [f"--property=WorkingDirectory={cwd}"]
        if mem_gb:
            props.append(f"--property=MemoryMax={int(mem_gb * 1024)}M")
        if timeout_h:
            props.append(f"--property=RuntimeMaxSec={int(timeout_h * 3600)}")
        argv = ["systemd-run", "--user", "--collect", "--quiet", f"--unit={unit}", *props,
                self.python, str(taskwrap), str(launch_path)]
        done = self.transport.run(shlex.join(argv) + "\n", timeout=30)
        if done.returncode:
            raise LaunchError(f"systemd-run failed: {done.stderr.strip()[-400:]}", done.returncode)

    def alive_check(self, unit, lab_dir):
        return (f'state=$(systemctl --user is-active "{unit}.service" 2>/dev/null); '
                f'case "$state" in active|activating|deactivating|reloading) echo "{unit} ALIVE";; '
                f'*) echo "{unit} DEAD";; esac')

    def stop(self, unit, lab_dir):
        self.transport.run(f"systemctl --user stop {shlex.quote(unit)}.service 2>/dev/null; "
                           f"systemctl --user reset-failed {shlex.quote(unit)}.service 2>/dev/null; true\n")


class SetsidLauncher(Launcher):
    """Fallback only: a session leader plus pid files, polled through /proc."""
    name = "setsid"

    def start(self, unit, launch_path, lab_dir, cwd, mem_gb=None, timeout_h=None, taskwrap=None):
        q = shlex.quote
        script = (f"mkdir -p {q(str(lab_dir))} && cd {q(str(cwd))} && "
                  f"setsid -f {q(self.python)} {q(str(taskwrap))} {q(str(launch_path))} "
                  f"</dev/null >>{q(str(lab_dir))}/wrap.log 2>&1\n")
        done = self.transport.run(script, timeout=30)
        if done.returncode:
            raise LaunchError(f"setsid launch failed: {done.stderr.strip()[-400:]}", done.returncode)

    def alive_check(self, unit, lab_dir):
        # The wrapper writes pid.json right after starting its child; before that a fresh
        # launch is still alive, so count a missing pid file younger than 60 s as alive.
        return (f'p=$(sed -n \'s/.*"wrapper": \\([0-9]*\\).*/\\1/p\' "{lab_dir}/pid.json" 2>/dev/null); '
                f'if [ -n "$p" ] && grep -q taskwrap /proc/$p/cmdline 2>/dev/null; then echo "{unit} ALIVE"; '
                f'elif [ -z "$p" ] && [ -n "$(find "{lab_dir}" -maxdepth 1 -name launch.json -newermt "-60 seconds" 2>/dev/null)" ]; '
                f'then echo "{unit} ALIVE"; else echo "{unit} DEAD"; fi')

    def stop(self, unit, lab_dir):
        q = shlex.quote(str(lab_dir))
        self.transport.run(
            f'for i in $(seq 50); do [ -f {q}/pid.json ] && break; [ -f {q}/rc.json ] && break; sleep 0.1; done; '
            f'p=$(sed -n \'s/.*"wrapper": \\([0-9]*\\).*/\\1/p\' {q}/pid.json 2>/dev/null); '
            f'c=$(sed -n \'s/.*"child": \\([0-9]*\\).*/\\1/p\' {q}/pid.json 2>/dev/null); '
            '[ -n "$p" ] && kill -TERM "$p" 2>/dev/null; '
            'for i in 1 2 3 4 5 6 7 8 9 10; do [ -n "$p" ] && kill -0 "$p" 2>/dev/null || break; sleep 0.5; done; '
            '[ -n "$c" ] && kill -KILL -- -"$c" 2>/dev/null; [ -n "$p" ] && kill -KILL "$p" 2>/dev/null; true\n')


LAUNCHERS = {"systemd": SystemdLauncher, "setsid": SetsidLauncher}


def systemd_user_works(transport):
    """User units work and survive the session only with a running manager and linger
    (or locally, where the desktop session keeps the manager alive)."""
    script = ('systemctl --user show-environment >/dev/null 2>&1 && echo manager-ok; '
              'loginctl show-user "$USER" -p Linger 2>/dev/null; true\n')
    done = transport.run(script, timeout=20)
    if done.returncode:
        return False, done.stderr.strip()
    out = done.stdout
    manager = "manager-ok" in out
    linger = "Linger=yes" in out
    return manager and (linger or not transport.remote), out.strip()
