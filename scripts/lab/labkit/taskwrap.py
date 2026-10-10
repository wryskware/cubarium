#!/usr/bin/env python3
"""Run one labd task on the execution host (standalone: stdlib, no labkit imports).

    taskwrap.py LAUNCH.json

It scrubs display variables (no task can reach a compositor), applies idle priority
(the equivalent of `chrt -i 0 nice -n 19`), confines the child to its L3 group, and
writes rc.json atomically when the child ends. labd reads rc.json to reconcile, so a
daemon restart never loses a result. CPU numbers for a group come from this host's
own sysfs (run-study.py beside it) unless the launch names explicit local CPUs.
"""

import importlib.util
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import sys
import threading
import time

DISPLAY_VARS = ("DISPLAY", "WAYLAND_SOCKET", "HYPRLAND_INSTANCE_SIGNATURE", "XAUTHORITY")


def atomic(path, text):
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_text(text)
    temporary.replace(path)


def group_cpus(spec):
    if spec.get("cpus"):
        return set(spec["cpus"])
    if spec.get("group") is None:
        return None
    module_spec = importlib.util.spec_from_file_location("run_study", spec["run_study"])
    module = importlib.util.module_from_spec(module_spec)
    module_spec.loader.exec_module(module)
    groups = module.l3_groups(os.sched_getaffinity(0))
    return set(groups[spec["group"] % len(groups)])


def verify(pid, cpus, log):
    time.sleep(2)
    for task in Path(f"/proc/{pid}/task").glob("*"):
        try:
            if not os.sched_getaffinity(int(task.name)) <= cpus:
                log.write(f"warning: thread {task.name} widened its affinity beyond {sorted(cpus)}\n")
        except (ProcessLookupError, OSError):
            pass
    log.flush()


def main():
    spec = json.loads(Path(sys.argv[1]).read_text())
    lab = Path(spec["lab_dir"])
    lab.mkdir(parents=True, exist_ok=True)
    rc_path = lab / "rc.json"
    rc_path.unlink(missing_ok=True)
    log = (lab / "wrap.log").open("a")
    env = os.environ.copy()
    for name in DISPLAY_VARS + tuple(spec.get("env_unset", [])):
        env.pop(name, None)
    # An unset WAYLAND_DISPLAY falls back to wayland-0, which may be the live compositor.
    env["WAYLAND_DISPLAY"] = "labd-no-display"
    env["CLAUDECODE"] = "1"        # cubarium refuses windows unless hidden.sh launched it
    env["LABD_TASK"] = "1"
    env.update({k: str(v) for k, v in spec.get("env", {}).items()})
    argv = list(spec["argv"])
    if spec.get("hidden_script"):
        argv = [spec["hidden_script"], *argv]
    cpus = group_cpus(spec)

    def prepare():
        if cpus:
            os.sched_setaffinity(0, cpus)
        if spec.get("idle"):
            os.setpriority(os.PRIO_PROCESS, 0, 19)
            os.sched_setscheduler(0, os.SCHED_IDLE, os.sched_param(0))
        if spec.get("rlimit_as_gb"):
            limit = int(float(spec["rlimit_as_gb"]) * 1024**3)
            resource.setrlimit(resource.RLIMIT_AS, (limit, limit))

    for key in ("stdout", "stderr"):
        Path(spec[key]).parent.mkdir(parents=True, exist_ok=True)
    cancelled = []
    with open(spec["stdout"], "ab") as out, open(spec["stderr"], "ab") as err:
        try:
            child = subprocess.Popen(argv, cwd=spec.get("cwd") or None, env=env, stdin=subprocess.DEVNULL,
                                     stdout=out, stderr=err, preexec_fn=prepare, start_new_session=True)
        except OSError as error:
            err.write(f"labd: cannot start {argv[0]}: {error}\n".encode())
            atomic(rc_path, json.dumps(dict(rc=127, ended=time.time(), error=str(error))))
            return 127
        atomic(lab / "pid.json", json.dumps(dict(wrapper=os.getpid(), child=child.pid)))
        log.write(f"started pid {child.pid} cpus={sorted(cpus) if cpus else 'inherited'} idle={bool(spec.get('idle'))}\n")
        log.flush()

        def forward(signum, _frame):
            cancelled.append(signum)
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass

        for signum in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
            signal.signal(signum, forward)
        if cpus:
            threading.Thread(target=verify, args=(child.pid, cpus, log), daemon=True).start()
        while True:
            try:
                rc = child.wait()
                break
            except InterruptedError:
                continue
    atomic(rc_path, json.dumps(dict(rc=rc, ended=time.time(), cancelled=bool(cancelled))))
    log.write(f"ended rc={rc}\n")
    log.close()
    return 0 if rc == 0 else min(abs(rc), 255) or 1


if __name__ == "__main__":
    sys.exit(main())
