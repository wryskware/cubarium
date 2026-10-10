"""File inputs shared by the remote study kinds (stdlib only)."""

from pathlib import Path
import json
import os
import subprocess
import tomllib

from ..core import SpecError


def references(value):
    """Config include/path fields, including nested tables and lists."""
    if isinstance(value, list):
        for item in value:
            yield from references(item)
        return
    if not isinstance(value, dict):
        return
    for key, item in value.items():
        if isinstance(item, dict) or isinstance(item, list) and any(isinstance(v, dict) for v in item):
            yield from references(item)
        elif key in ("include", "includes", "path", "file") or key.endswith(("_path", "_file")):
            for path in item if isinstance(item, list) else [item]:
                if isinstance(path, str) and path:
                    yield path


def pinned_config(tree, commit, relative, root):
    """Materialize a config and its references from this arm's exact Git commit."""
    tree, root = Path(tree), Path(root)
    seen = set()

    def copy(path):
        path = Path(path)
        path = Path(os.path.normpath(str(path)))
        if path.is_absolute() or ".." in path.parts:
            raise SpecError(f"config input escapes its pinned tree: {path}")
        if path in seen:
            return root / path
        content = subprocess.check_output(["git", "-C", str(tree), "show", f"{commit}:{path.as_posix()}"],
                                          stderr=subprocess.PIPE, timeout=20)
        seen.add(path)
        target = root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        if path.suffix == ".toml":
            text = content.decode()
            for ref in references(tomllib.loads(content.decode())):
                dependency = Path(ref)
                if dependency.is_absolute():
                    dependency = dependency.relative_to(tree)
                    child = copy(dependency)
                else:
                    candidate = Path(os.path.normpath(str(path.parent / dependency)))
                    try:
                        child = copy(candidate)
                    except subprocess.CalledProcessError:
                        child = copy(dependency)
                relative_ref = os.path.relpath(child, target.parent)
                text = text.replace(json.dumps(ref), json.dumps(relative_ref))
                text = text.replace("'" + ref + "'", "'" + relative_ref + "'")
            content = text.encode()
        target.write_bytes(content)
        return target

    return copy(relative)


def file_inputs(path, destination, files):
    """Preserve relative config references when staging a submitter-supplied file."""
    path = Path(path).resolve(strict=True)
    if str(path) in files:
        return
    files[str(path)] = destination
    if path.suffix == ".toml":
        for ref in references(tomllib.loads(path.read_text())):
            child = Path(ref)
            if child.is_absolute():
                raise SpecError(f"remote config input must use a relative path: {ref}")
            file_inputs(path.parent / child, str(Path(destination).parent / child), files)
