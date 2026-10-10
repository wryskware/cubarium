"""cargo-test: a test suite at one exact commit, in a snapshot tree, with its exit status.

    kind = "cargo-test"
    repo = "."
    commit = "working"     # or a ref / sha
    command = ["cargo", "nextest", "run", "--workspace", "--exclude", "cubarium-gpu"]
    profile = "debug"      # which target/<profile> to seed by reflink
"""

from ..core import Kind, SpecError, Task
from .build import pin, seeds


class CargoTest(Kind):
    name = "cargo-test"
    description = "cargo test/nextest suite at a commit (snapshot tree), detached"

    def validate(self, spec, ctx):
        pin(spec, ctx)
        spec.setdefault("command", ["cargo", "nextest", "run", "--workspace", "--exclude", "cubarium-gpu"])
        if not isinstance(spec["command"], list) or spec["command"][:1] != ["cargo"]:
            raise SpecError("command must be a cargo argv list")
        spec.setdefault("profile", "debug")
        spec.setdefault("threads", ctx.inventory.build.get("threads", 16) if ctx.inventory else 16)
        spec.setdefault("mem_gb", ctx.inventory.build.get("mem_gb", 16) if ctx.inventory else 16)
        return spec

    def plan(self, job, ctx):
        spec = job.spec
        trees = ctx.paths.cache / "trees"
        argv = [ctx.python, str(ctx.lab_dir / "snaprun.py"), "--repo", spec["repo"], "--commit", spec["commit"],
                "--trees", str(trees), *[a for s in seeds(ctx, spec["repo"]) for a in ("--seed", s)],
                "--profile", spec["profile"], "--out", "{out}", "--", *spec["command"]]
        return [Task(name="test", argv=argv, threads=spec["threads"], mem_gb=spec["mem_gb"], local_only=True,
                     est_h=spec.get("est_h", 0.25), timeout_h=spec.get("timeout_h", 1),
                     env={"CARGO_BUILD_JOBS": str(spec["threads"])},
                     meta=dict(tree=str(trees / spec["commit"][:16])))]

    def finish(self, job, ctx):
        view = job.tasks.get("test")
        if view is None:
            return "no test task"
        tail = ""
        try:
            tail = "\n".join((view.out / "stderr.log").read_text(errors="replace").splitlines()[-15:])
        except OSError:
            pass
        verdict = "passed" if view.state == "done" else f"FAILED ({view.state}, exit {view.rc})"
        dirty = " (dirty snapshot)" if job.spec.get("dirty") else ""
        return f"tests {verdict} at {job.spec['commit'][:12]}{dirty}\n{tail}"
