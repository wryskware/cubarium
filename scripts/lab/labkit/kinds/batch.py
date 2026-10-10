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

import glob
import json
from pathlib import Path
import shlex
import shutil

from ..core import Kind, SpecError, Task
from .staging import file_inputs, pinned_config

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
        for arg in args:
            if arg.startswith("--threads="):
                threads = int(arg.split("=", 1)[1])
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
        output = job.out / "batch-task"
        out = str(output / "batch")
        if spec.get("resume"):
            output.mkdir(parents=True, exist_ok=True)
            shutil.copytree(spec["resume"], out, dirs_exist_ok=True)
        files = {}
        if spec.get("bin"):
            files[binary] = "{job}/inputs/binary"
        if spec.get("resume"):
            manifest_path = Path(out) / "batch.json"
            manifest = json.loads(manifest_path.read_text())
            previous_out = manifest["args"]["out"]
            paths = manifest.get("lab_paths", {})
            for local, remote in sorted(paths.items(), key=lambda p: len(p[1]), reverse=True):
                previous_out = previous_out.replace(remote, local)

            def localize(value):
                if isinstance(value, str):
                    for local, remote in sorted(paths.items(), key=lambda p: len(p[1]), reverse=True):
                        value = value.replace(remote, local)
                    return value.replace(previous_out, out)
                if isinstance(value, list):
                    return [localize(v) for v in value]
                if isinstance(value, dict):
                    return {localize(k): localize(v) for k, v in value.items()}
                return value

            manifest = localize(manifest)
            manifest["lab_paths"] = {}
            manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
            identity = manifest["identity"]
            sources = [item["path"] for item in (identity.get("config"), identity.get("required")) if item]
            sources += list(manifest["starts"].values()) + list(identity.get("genomes", {}))
            sources += [item["path"] for policies in identity["policies"].values() for item in policies.values()]
            for number, source in enumerate(sources):
                file_inputs(source, f"{{job}}/inputs/resume/{number}/{Path(source).name}", files)
        args = list(spec["args"])
        for index, arg in enumerate(args):
            flag, equals, value = arg.partition("=")
            if flag in ("--config", "--required", "--starts"):
                value = value if equals else args[index + 1]
                source = ctx.path(spec, value)
                destination = "{job}/inputs/" + flag[2:] + "/" + source.name
                if flag == "--starts":
                    matches = sorted(glob.glob(str(source)))
                    if not matches:
                        raise SpecError("--starts matched no checkpoints")
                    from batch import start_seed
                    for match in matches:
                        checkpoint = Path(match).resolve(strict=True)
                        seed = start_seed(checkpoint)
                        staged = job.out / "inputs/starts" / f"s{seed}" / "checkpoint"
                        staged.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copytree(checkpoint, staged, dirs_exist_ok=True)
                        files[str(staged)] = f"{{job}}/inputs/starts/s{seed}/checkpoint"
                        # batch.py discovers the founding roster/census beside the checkpoint.
                        census = next((parent / "census.csv" for parent in checkpoint.parents
                                       if (parent / "census.csv").is_file()), None)
                        if census:
                            shutil.copy2(census, staged.parent / "census.csv")
                            files[str(staged.parent / "census.csv")] = f"{{job}}/inputs/starts/s{seed}/census.csv"
                    value = str(job.out / "inputs/starts/*/checkpoint")
                else:
                    if flag == "--config" and spec.get("binary"):
                        pinned = spec["binary"]
                        relative = source.resolve().relative_to(Path(pinned["tree"]))
                        bundle = job.out / "inputs/config-tree"
                        source = pinned_config(pinned["tree"], pinned["commit"], relative, bundle)
                        files[str(bundle)] = "{job}/inputs/config-tree"
                    else:
                        file_inputs(source, destination, files)
                    value = str(source)
                if equals:
                    args[index] = flag + "=" + value
                else:
                    args[index + 1] = value
            elif flag == "--arm":
                value = value if equals else args[index + 1]
                name, _, extra = value.partition("=")
                tokens = shlex.split(extra)
                for token_index, token in enumerate(tokens):
                    if token.startswith(("policy=", "genome=", "load=")):
                        prefix, _, path = token.rpartition("=")
                        source = ctx.path(spec, path).resolve(strict=True)
                        file_inputs(source, f"{{job}}/inputs/arm-{name}/{source.name}", files)
                        tokens[token_index] = prefix + "=" + str(source)
                value = name + "=" + shlex.join(tokens)
                if equals:
                    args[index] = flag + "=" + value
                else:
                    args[index + 1] = value
        parallel, threads = int(spec["parallel"]), spec["threads_per_world"]
        resume = ["{python}", script, out, "--bin", binary, "--resume", "--max-parallel", str(parallel),
                  "--threads", str(threads)]
        argv = resume if spec.get("resume") else ["{python}", script, out, "--bin", binary,
                                                   "--max-parallel", str(parallel), *args]
        return [Task(name="batch", argv=argv, threads=parallel * threads, mem_gb=spec.get("mem_gb", 3 * parallel),
                     l3=True, arch=ctx.arch, hosts=spec.get("hosts"), inputs=inputs, files=files,
                     outputs=str(output), retries=1, resume_argv=resume,
                     liveness=dict(paths=["batch/*/s*/census.csv", "batch/STATE.tsv"],
                                   stall_min=spec.get("stall_min", 30)),
                     est_h=spec.get("est_h"), timeout_h=spec.get("timeout_h"), meta=dict(out=out, batch=True))]

    def census_runs(self, job, ctx):
        from ..stops import CensusRun
        root = job.out / "batch-task/batch"
        if not job.tasks and job.spec.get("resume"):
            root = Path(job.spec["resume"])
        try:
            manifest = json.loads((root / "batch.json").read_text())
        except FileNotFoundError:
            return []
        return [CensusRun(run["arm"], str(run["seed"]),
                          [root / run["arm"] / f"s{run['seed']}" / "census.csv"])
                for run in manifest["runs"]]

    def finish(self, job, ctx):
        view = job.tasks.get("batch")
        out = view.task.meta["out"] if view else "?"
        counts = {}
        try:
            for line in Path(out, "STATE.tsv").read_text().splitlines()[1:]:
                state = line.split("\t")[2]
                counts[state] = counts.get(state, 0) + 1
        except (OSError, IndexError):
            pass
        states = ", ".join(f"{n} {s}" for s, n in sorted(counts.items())) or "no STATE.tsv"
        return f"batch {view.state if view else '?'} (exit {view.rc if view else '-'}): runs {states}; out {out}"
