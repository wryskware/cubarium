"""command: any headless argv with resources (the escape hatch).

    kind = "command"
    argv = ["target/release/examples/voxel_census", "1", "seed=3", "run={out}/run"]
    threads = 1          mem_gb = 2         l3 = true       est_h = 1.5
    hosts = ["desktop"]  # or ["eidolon"], ["ec2"]; default: local hosts
    binary = { commit = "HEAD", name = "voxel_census" }   # optional: {bin} in argv
    liveness = { stall_min = 30, paths = ["run/status"] }
    done_marker = "run/DONE"     hidden = false    timeout_h = 3
    quick = true         # quick lane (`lab quick -- argv...` writes this spec for you)

Placeholders: {out} task outputs, {job} job dir, {bin} the requested binary,
{input:KEY} any artifact, {python}, {lab}. Display variables are scrubbed for every
task; a window needs hidden = true (scripts/hidden.sh, local host only).
"""

from pathlib import Path

from ..core import Kind, SpecError, Task

FIELDS = ("threads", "mem_gb", "l3", "arch", "hosts", "local_only", "done_marker", "cwd", "env", "stdout",
          "stderr", "hidden", "liveness", "timeout_h", "est_h", "retries", "resume_argv", "produces",
          "rlimit_as_gb", "inputs", "quick")
WINDOWED = ("window",)


def headless_check(argv, hidden):
    for flag, value in zip(argv, argv[1:]):
        if flag in ("--gpu-target", "--sink-target") and value in WINDOWED and not hidden:
            raise SpecError("a windowed target needs hidden = true (scripts/hidden.sh); tasks are headless")


def local_paths(argv, cwd, files):
    """argv tokens (or the value of key=value / --flag=value) naming an existing local file:
    relative to the task's local cwd, or absolute under $HOME (a worktree binary, local data).
    Placeholders and staged `files` are portable, and system paths exist on every host."""
    home = Path.home()
    found = []
    for arg in argv:
        for token in {arg, arg.partition("=")[2]}:
            if not token or "{" in token or token in files:
                continue
            path = Path(token)
            if not path.is_absolute():
                if not cwd or "/" not in token:   # bare words (true, python3, seed=3) are not paths
                    continue
                path = Path(cwd) / path
            elif not path.is_relative_to(home):
                continue
            if path.exists():
                found.append(token)
    return found


class Command(Kind):
    name = "command"
    description = "any headless argv with resources"

    def validate(self, spec, ctx):
        argv = ctx.require(spec, "argv", list)
        if not argv or not all(isinstance(a, str) for a in argv):
            raise SpecError("argv must be a nonempty list of strings")
        if any(k in spec.get("env", {}) for k in ("DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET")):
            raise SpecError("tasks may not set display variables")
        headless_check(argv, spec.get("hidden", False))
        binary = spec.get("binary")
        if binary:
            if not isinstance(binary, dict):
                raise SpecError("binary must be a table {commit, name, tree}")
            tree = ctx.path(spec, binary.get("tree", "."))
            if not binary.get("pinned"):
                # commit omitted or "working": the working tree (a dirty tree is snapshotted);
                # any ref (HEAD, a sha, a branch) pins that commit.
                ref = binary.get("commit")
                pinned = ctx.resolve_commit(tree, None if ref in (None, "working") else ref)
                binary.update(tree=pinned["tree"], commit=pinned["commit"], dirty=pinned["dirty"], pinned=True)
            binary.setdefault("name", "voxel_census")
        for key in ("threads", "mem_gb", "est_h", "timeout_h"):
            if key in spec and not isinstance(spec[key], (int, float)):
                raise SpecError(f"{key} must be a number")
        Task(name="probe", argv=argv, **{k: spec[k] for k in FIELDS if k in spec})  # field validation
        return spec

    def plan(self, job, ctx):
        spec = job.spec
        fields = {k: spec[k] for k in FIELDS if k in spec}
        argv = list(spec["argv"])
        binary = spec.get("binary")
        if binary:
            key = ctx.binary(binary["tree"], binary["commit"], binary["name"], arch=binary.get("arch"),
                             package=binary.get("package"), example=binary.get("example"), bin=binary.get("bin"),
                             dirty=binary.get("dirty", False))
            argv = [a.replace("{bin}", "{input:" + key + "}") for a in argv]
            fields["inputs"] = list(fields.get("inputs", [])) + [key]
        cwd = spec.get("cwd")
        hosts = spec.get("hosts")
        local = ctx.inventory is None or hosts is None or all(
            getattr(ctx.inventory.hosts.get(h), "kind", "remote") == "local" for h in hosts)
        if cwd and "{" not in cwd:
            fields["cwd"] = str(ctx.path(spec, cwd))
        elif not cwd:
            fields["cwd"] = str(ctx.repo) if local else None   # remote default: {out}
        if (cwd and "{" not in cwd) or local_paths(argv, fields.get("cwd"), fields.get("files", {})):
            fields["meta"] = dict(fields.get("meta", {}), local_inputs=True)   # placement: local hosts first
        return [Task(name=spec.get("task", "run"), argv=argv, **fields)]

    def finish(self, job, ctx):
        view = next(iter(job.tasks.values()), None)
        if view is None:
            return "no task"
        return f"{view.name} {view.state} rc={view.rc} on {view.host}; outputs {view.out}"
