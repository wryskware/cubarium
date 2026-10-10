"""batch: run scripts/studies/batch.py (policies, starts, --resume) under the service.

    kind = "batch"
    parallel = 8                    # worlds at once (batch.py --max-parallel)
    args = ["--arm", "base=", "--arm", "cand=set=flora.x=2", "--config", "config/desktop/terrarium-wide.toml",
            "--required", "config/scenarios/required.toml", "--seeds", "1-8", "--hours", "2"]
    bin = "runs/bins/voxel_census.66d60783"      # or binary = { commit = "HEAD" } (cached build)
    resume = "runs/lab/<old-job>/batch"          # optional: continue an interrupted batch instead

batch.py keeps its own L3-slot placement through run-study.py, inside the CPU set labd
gives the task, and its own staged runs and liveness checks. labd charges it
parallel x threads, watches census growth, and retries a lost batch once with --resume.
"""

from ..core import Kind, SpecError, Task

OWNED = ("--detach", "--bin", "--resume", "--max-parallel")


class Batch(Kind):
    name = "batch"
    description = "scripts/studies/batch.py batch (library/search/policy batches) under the service"

    def validate(self, spec, ctx):
        args = [str(a) for a in spec.get("args", [])]
        if any(a.split("=", 1)[0] in OWNED for a in args):
            raise SpecError(f"labd owns {', '.join(OWNED)}; use bin/binary, parallel and resume fields")
        if not spec.get("resume") and not args:
            raise SpecError("a new batch needs args (arms, config or starts, hours); or resume = DIR")
        spec["args"] = args
        threads = 1
        if "--threads" in args:
            threads = int(args[args.index("--threads") + 1])
        spec["threads_per_world"] = threads
        spec.setdefault("parallel", 8)
        if int(spec["parallel"]) < 1:
            raise SpecError("parallel must be >= 1")
        if spec.get("bin"):
            spec["bin"] = str(ctx.path(spec, spec["bin"]).resolve(strict=True))
        elif isinstance(spec.get("binary"), dict) and not spec["binary"].get("pinned"):
            binary = spec["binary"]
            ref = binary.get("commit")
            pinned = ctx.resolve_commit(ctx.path(spec, binary.get("tree", ".")), None if ref in (None, "working") else ref)
            binary.update(tree=pinned["tree"], commit=pinned["commit"], dirty=pinned["dirty"], pinned=True)
        elif not spec.get("binary"):
            raise SpecError("batch needs bin = PATH or binary = {commit = ...}")
        if spec.get("resume"):
            spec["resume"] = str(ctx.path(spec, spec["resume"]).resolve(strict=True))
        return spec

    def plan(self, job, ctx):
        spec = job.spec
        inputs = []
        if spec.get("bin"):
            binary = spec["bin"]
        else:
            key = ctx.binary(spec["binary"]["tree"], spec["binary"]["commit"], spec["binary"].get("name", "voxel_census"),
                             dirty=spec["binary"].get("dirty", False))
            binary, inputs = "{input:" + key + "}", [key]
        script = str(ctx.repo / "scripts/studies/batch.py")
        out = spec.get("resume") or str(job.out / "batch")
        parallel, threads = int(spec["parallel"]), spec["threads_per_world"]
        resume = [ctx.python, script, out, "--bin", binary, "--resume", "--max-parallel", str(parallel),
                  "--threads", str(threads)]
        argv = resume if spec.get("resume") else [ctx.python, script, out, "--bin", binary,
                                                   "--max-parallel", str(parallel), *spec["args"]]
        return [Task(name="batch", argv=argv, threads=parallel * threads, mem_gb=spec.get("mem_gb", 3 * parallel),
                     local_only=True, inputs=inputs, cwd=str(ctx.repo), retries=1, resume_argv=resume,
                     liveness=dict(paths=[f"{out}/*/s*/census.csv", f"{out}/STATE.tsv"],
                                   stall_min=spec.get("stall_min", 30)),
                     est_h=spec.get("est_h"), timeout_h=spec.get("timeout_h"), meta=dict(out=out))]

    def finish(self, job, ctx):
        view = job.tasks.get("batch")
        out = view.task.meta["out"] if view else "?"
        counts = {}
        try:
            for line in open(f"{out}/STATE.tsv").read().splitlines()[1:]:
                state = line.split("\t")[2]
                counts[state] = counts.get(state, 0) + 1
        except (OSError, IndexError):
            pass
        states = ", ".join(f"{n} {s}" for s, n in sorted(counts.items())) or "no STATE.tsv"
        return f"batch {view.state if view else '?'} (exit {view.rc if view else '-'}): runs {states}; out {out}"
