"""build: cargo build at one exact commit, cached as binary:<commit>:<arch>:<target>.

    kind = "build"
    repo = "."               # any tree of the repository (worktrees share objects)
    commit = "working"       # default: the working tree, a dirty one snapshotted by
                             # `git stash create`; or any ref / sha
    target = "voxel_census"  # recipe: package + example/bin; or give package/example/bin
    arch = "x86_64"          # aarch64 builds natively on an approved EC2 host

The build runs scripts/lab/labkit/snaprun.py in a detached worktree under the cache's
trees/ (never the live checkout), seeding target/<profile> by reflink. Other kinds never
submit builds themselves: they name the key (ctx.binary) and the core plans this kind.
"""

import functools
import json
import os
import re
import shutil
import subprocess

from ..core import Kind, SpecError, Task, binary_key

RECIPES = {
    "voxel_census": dict(package="cubarium", example="voxel_census"),
    "cubarium": dict(package="cubarium", bin="cubarium"),
    "cubarium-search": dict(package="cubarium-search", bin="cubarium-search"),
}
SOURCE_EXCLUDES = ("art", "design", "captures", "runs", "graft", "docs", "spikes", "assets/atelier",
                   "assets/voxel-textures")


@functools.lru_cache(maxsize=1)
def local_toolchain():
    """This machine's rustc version (for example "1.98.1"), so a native build on another arch uses
    the same compiler as the local one: the workspace denies warnings, and a newer rustc's
    deprecations fail the build (first EC2 launch, 2026-10-10). "stable" if rustc can't be read."""
    for rustc in (shutil.which("rustc"), os.path.expanduser("~/.cargo/bin/rustc")):
        if not rustc:
            continue
        try:
            out = subprocess.run([rustc, "--version"], capture_output=True, text=True, timeout=10).stdout
        except (OSError, subprocess.SubprocessError):
            continue
        match = re.match(r"rustc (\d+\.\d+\.\d+)", out)
        if match:
            return match.group(1)
    return "stable"


def pin(spec, ctx, field="commit"):
    tree = ctx.path(spec, spec.get("repo", "."))
    if not spec.get("pinned"):
        ref = spec.get(field)
        pinned = ctx.resolve_commit(tree, None if ref in (None, "working") else ref)
        spec.update(repo=pinned["tree"], commit=pinned["commit"], dirty=pinned["dirty"], pinned=True)
    return spec


def seeds(ctx, repo):
    listed = ctx.inventory.build.get("seed_targets", ["target"]) if ctx.inventory else ["target"]
    result = []
    for item in listed:
        for base in (repo, ctx.repo):
            path = ctx.path({"_cwd": str(base)}, item)
            if str(path) not in result:
                result.append(str(path))
    return result


def cargo_args(spec):
    profile = spec.get("profile", "release")
    argv = ["cargo", "build", "--locked"] if spec.get("locked") else ["cargo", "build"]
    argv += ["--release"] if profile == "release" else ["--profile", profile]
    argv += ["-p", spec["package"]]
    argv += ["--example", spec["example"]] if spec.get("example") else ["--bin", spec["bin"]]
    if spec.get("features"):
        argv += ["--features", ",".join(spec["features"])]
    argv += ["-j", str(spec["threads"])]
    relative = f"{profile}/examples/{spec['example']}" if spec.get("example") else f"{profile}/{spec['bin']}"
    return argv, relative


def source_key(ctx, repo, commit):
    key = f"source:{commit}"
    if ctx.store:
        ctx.store.x("INSERT INTO recipes(key, spec) VALUES(?, ?) ON CONFLICT(key) DO NOTHING", key,
                    json.dumps(dict(provider="source", repo=repo, commit=commit)))
    return key


def simulation_arch(spec, ctx):
    """Keep a simulation job on one platform; an EC2 opt-in selects its native arch."""
    if "ec2" not in (spec.get("hosts") or []):
        return ctx.arch
    cfg = ctx.inventory.ec2 if ctx.inventory else {}
    instance = (cfg.get("types") or ["c7g.16xlarge"])[0]
    return cfg.get("instance_types", {}).get(instance, {}).get("arch", "aarch64")


class Build(Kind):
    name = "build"
    description = "cargo build at a commit into the artifact cache"

    def validate(self, spec, ctx):
        pin(spec, ctx)
        spec.setdefault("arch", ctx.arch)
        spec.setdefault("target", "voxel_census")
        recipe = RECIPES.get(spec["target"], {})
        for key, value in recipe.items():
            if not any(spec.get(k) for k in ("example", "bin")) or key == "package":
                spec.setdefault(key, value)
        if not spec.get("package") or not (spec.get("example") or spec.get("bin")):
            raise SpecError(f"unknown target {spec['target']!r}: give package and example or bin")
        threads = ctx.inventory.build.get("threads", 16) if ctx.inventory else 16
        spec.setdefault("threads", threads)
        spec.setdefault("mem_gb", ctx.inventory.build.get("mem_gb", 16) if ctx.inventory else 16)
        return spec

    def key(self, spec):
        return binary_key(spec["commit"], spec["arch"], spec["target"])

    def plan(self, job, ctx):
        spec = job.spec
        key = ctx.binary(spec["repo"], spec["commit"], spec["target"], arch=spec["arch"],
                         package=spec.get("package"), example=spec.get("example"), bin=spec.get("bin"),
                         dirty=spec.get("dirty", False))
        cached = ctx.artifact(key)
        if cached:
            job.state["cached"] = str(cached)
            return []
        cargo, relative = cargo_args(spec)
        trees = ctx.paths.cache / "trees"
        common = dict(threads=spec["threads"], mem_gb=spec["mem_gb"], produces={key: f"artifact/{spec['target']}"},
                      est_h=spec.get("est_h", 0.25), timeout_h=spec.get("timeout_h", 2), priority=10,
                      meta=dict(tree=str(trees / spec["commit"][:16]), key=key, kind="build"))
        if spec["arch"] == ctx.arch:
            argv = [ctx.python, str(ctx.lab_dir / "snaprun.py"), "--repo", spec["repo"], "--commit", spec["commit"],
                    "--trees", str(trees), *[a for s in seeds(ctx, spec["repo"]) for a in ("--seed", s)],
                    "--profile", spec.get("profile", "release"), "--artifact", f"{spec['target']}={relative}",
                    "--out", "{out}", "--", *cargo]
            return [Task(name="build", argv=argv, local_only=True, arch=ctx.arch, **common)]
        # Another arch builds natively on its host from a source archive of the same commit.
        source = source_key(ctx, spec["repo"], spec["commit"])
        # The same rustc as the local build (spec "toolchain" overrides), installed by rustup.
        toolchain = spec.get("toolchain") or local_toolchain()
        bootstrap = ('. "$HOME/.cargo/env" 2>/dev/null; '
                     'command -v rustup >/dev/null || { curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal '
                     '--default-toolchain none >/dev/null && . "$HOME/.cargo/env"; }; '
                     f'rustup toolchain install {toolchain} --profile minimal >/dev/null && '
                     f'export RUSTUP_TOOLCHAIN={toolchain} && exec "$@"')
        argv = ["bash", "-c", bootstrap, "labd-build", "{python}", "{lab}/snaprun.py", "--commit", spec["commit"],
                "--trees", "{lab}/../trees", "--source-tar", "{input:" + source + "}", "--profile",
                spec.get("profile", "release"), "--artifact", f"{spec['target']}={relative}", "--out", "{out}",
                "--", *cargo]
        return [Task(name="build", argv=argv, arch=spec["arch"], hosts=["ec2"], inputs=[source], **common)]

    def finish(self, job, ctx):
        if job.state.get("cached"):
            return f"cached {self.key(job.spec)} at {job.state['cached']}"
        path = ctx.artifact(self.key(job.spec))
        dirty = " (dirty snapshot)" if job.spec.get("dirty") else ""
        return f"built {self.key(job.spec)}{dirty} at {path}" if path else "build produced no artifact"


def source(key, recipe, ctx):
    """source:<commit>: a `git archive` of tracked files, minus bulk dirs (aws-graviton.sh's list)."""
    commit = recipe["commit"]
    argv = ["git", "-C", recipe["repo"], "archive", "--format=tar", "-o", "{out}/source.tar", commit, "--", "."]
    argv += [f":(exclude){d}" for d in SOURCE_EXCLUDES]
    return dict(kind="command", name=f"source-{commit[:8]}", argv=argv, local_only=True,
                produces={key: "source.tar"}, priority=10, _cwd=recipe["repo"])


PROVIDERS = {"source": source}
