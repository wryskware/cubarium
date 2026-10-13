"""A stand-in for the instance's `aws s3` cli: buckets are directories under $FAKE_AWS_ROOT.

Every invocation is appended to $FAKE_AWS_LOG as a JSON argv list. A bucket directory holding
a `.deny` file refuses everything (an instance without the role). Supports `s3 ls`, `s3 cp`
(file, `-` from stdin, `--recursive` trees, either direction) and `s3 rm --recursive
--exclude PATTERN`, which is all labd uses.
"""

import fnmatch
import json
import os
from pathlib import Path
import shutil
import sys


def main(argv):
    root = Path(os.environ["FAKE_AWS_ROOT"])
    with open(os.environ["FAKE_AWS_LOG"], "a") as log:
        log.write(json.dumps(argv) + "\n")
    words, excludes, recursive = [], [], False
    items = iter(argv)
    for word in items:
        if word == "--region":
            next(items)
        elif word == "--exclude":
            excludes.append(next(items))
        elif word == "--recursive":
            recursive = True
        elif word.startswith("--"):
            continue
        else:
            words.append(word)
    if words[:1] != ["s3"] or len(words) < 3:
        print(f"fake aws: unsupported {argv}", file=sys.stderr)
        return 2

    def local(uri):
        if not uri.startswith("s3://"):
            return None
        bucket, _, key = uri[5:].partition("/")
        if not (root / bucket).is_dir() or (root / bucket / ".deny").exists():
            raise PermissionError(f"An error occurred (AccessDenied) for {uri}")
        return root / bucket / key

    try:
        verb, rest = words[1], words[2:]
        if verb == "ls":
            local(rest[0])
            return 0
        if verb == "cp":
            source, target = rest[0], rest[1]
            src = local(source) if source.startswith("s3://") else (None if source == "-" else Path(source))
            dst = local(target) if target.startswith("s3://") else Path(target)
            if recursive:
                if not src.is_dir():
                    print(f"fake aws: nothing at {source}", file=sys.stderr)
                    return 1
                for path in src.rglob("*"):
                    if path.is_file():
                        out = dst / path.relative_to(src)
                        out.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copyfile(path, out)
                return 0
            if target.endswith("/"):
                dst = dst / Path(source).name
            dst.parent.mkdir(parents=True, exist_ok=True)
            if src is None:
                dst.write_bytes(sys.stdin.buffer.read())
            elif not src.is_file():
                print(f"fake aws: no such key {source}", file=sys.stderr)
                return 1
            else:
                shutil.copyfile(src, dst)
            return 0
        if verb == "rm" and recursive:
            prefix = local(rest[0])
            for path in sorted(prefix.rglob("*"), reverse=True) if prefix.is_dir() else []:
                relative = path.relative_to(prefix).as_posix()
                if path.is_file() and not any(fnmatch.fnmatch(relative, p) for p in excludes):
                    path.unlink()
                elif path.is_dir() and not any(path.iterdir()):
                    path.rmdir()
            return 0
    except PermissionError as error:
        print(str(error), file=sys.stderr)
        return 1
    print(f"fake aws: unsupported {argv}", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
