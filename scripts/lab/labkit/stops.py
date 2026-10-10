"""Job stop rules: a deliberately small AST interpreter, never Python eval."""

import ast
import csv
from dataclasses import dataclass
import io
import json
import math
import operator
from pathlib import Path
import subprocess

from .core import SpecError


@dataclass
class CensusRun:
    arm: str
    seed: str
    paths: list


class Waiting(Exception):
    """A run or the requested history has not arrived yet."""


OPS = {ast.Add: operator.add, ast.Sub: operator.sub, ast.Mult: operator.mul,
       ast.Div: operator.truediv, ast.Lt: operator.lt, ast.LtE: operator.le,
       ast.Gt: operator.gt, ast.GtE: operator.ge, ast.Eq: operator.eq, ast.NotEq: operator.ne}


def sim_minute(row):
    return float(row["sim_min"] if "sim_min" in row else row["minute"])


def identifier(node):
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Constant) and isinstance(node.value, str) and node.value:
        return node.value
    raise SpecError("delta/arm names must be column/arm identifiers or quoted names")


class Expression:
    def __init__(self, text, ab=False):
        try:
            self.tree = ast.parse(text, mode="eval").body
        except (SyntaxError, TypeError, ValueError, RecursionError) as error:
            raise SpecError(f"invalid stop expression: {error}") from error
        self.columns = set()
        self.peers = []
        self.ab = ab
        self.validate(self.tree)

    def validate(self, node):
        if isinstance(node, ast.Name):
            self.columns.add(node.id)
        elif isinstance(node, ast.Constant) and type(node.value) in (int, float) \
                and math.isfinite(node.value):
            pass
        elif isinstance(node, ast.BinOp) and type(node.op) in (ast.Add, ast.Sub, ast.Mult, ast.Div):
            self.validate(node.left)
            self.validate(node.right)
        elif isinstance(node, ast.UnaryOp) and type(node.op) in (ast.Not, ast.UAdd, ast.USub):
            self.validate(node.operand)
        elif isinstance(node, ast.BoolOp) and type(node.op) in (ast.And, ast.Or):
            for value in node.values:
                self.validate(value)
        elif isinstance(node, ast.Compare) and all(type(op) in OPS for op in node.ops):
            self.validate(node.left)
            for value in node.comparators:
                self.validate(value)
        elif isinstance(node, ast.Call) and isinstance(node.func, ast.Name) \
                and len(node.args) == 2 and not node.keywords:
            if node.func.id == "delta":
                self.columns.add(identifier(node.args[0]))
                window = node.args[1]
                if not isinstance(window, ast.Constant) or type(window.value) not in (int, float) \
                        or not math.isfinite(window.value) or window.value <= 0:
                    raise SpecError("delta minutes must be a positive number")
            elif node.func.id == "arm" and self.ab:
                self.peers.append((identifier(node.args[0]), identifier(node.args[1])))
            else:
                raise SpecError("only delta(col, minutes) and ab's arm(name, col) are allowed")
        else:
            raise SpecError(f"unsupported stop expression: {ast.dump(node)}")

    def headers(self, header, columns):
        unknown = set(columns) - set(header)
        if unknown:
            raise SpecError("unknown census column(s): " + ", ".join(sorted(unknown)))

    def value(self, history, peers):
        if not history:
            raise Waiting()
        row = history[-1]

        def number(value):
            result = float(value)
            if not math.isfinite(result):
                raise ValueError("non-finite census value")
            return result

        def visit(node):
            if isinstance(node, ast.Name):
                return number(row[node.id])
            if isinstance(node, ast.Constant):
                return node.value
            if isinstance(node, ast.BinOp):
                return OPS[type(node.op)](visit(node.left), visit(node.right))
            if isinstance(node, ast.UnaryOp):
                value = visit(node.operand)
                return not value if isinstance(node.op, ast.Not) else -value if isinstance(node.op, ast.USub) else value
            if isinstance(node, ast.BoolOp):
                values = (bool(visit(value)) for value in node.values)
                return all(values) if isinstance(node.op, ast.And) else any(values)
            if isinstance(node, ast.Compare):
                left = visit(node.left)
                for op, right in zip(node.ops, node.comparators):
                    right = visit(right)
                    if not OPS[type(op)](left, right):
                        return False
                    left = right
                return True
            if node.func.id == "arm":
                other = peers.get(identifier(node.args[0]), [])
                if not other:
                    raise Waiting()
                return number(other[-1][identifier(node.args[1])])
            column, window = identifier(node.args[0]), node.args[1].value
            boundary = sim_minute(row) - window
            earlier = [r for r in history if sim_minute(r) <= boundary]
            if not earlier:
                raise Waiting()
            return number(row[column]) - number(earlier[-1][column])

        return bool(visit(self.tree))


def census(run):
    """Ignore an in-flight final CSV line; combine stage history by sim minute."""
    headers, rows = [], {}
    for path in run.paths:
        try:
            text = Path(path).read_text()
        except FileNotFoundError:
            continue
        end = text.rfind("\n")
        if end < 0:
            continue
        reader = csv.DictReader(io.StringIO(text[:end + 1]))
        if reader.fieldnames:
            headers.append(reader.fieldnames)
        for row in reader:
            if None in row or any(value is None for value in row.values()):
                continue
            minute = sim_minute(row)
            if not math.isfinite(minute):
                raise ValueError(f"non-finite census minute in {path}")
            rows[minute] = row
    return headers, [rows[key] for key in sorted(rows)]


def validate_rules(spec, ctx, kind):
    rules = spec.get("stop", [])
    if not isinstance(rules, list) or any(not isinstance(rule, dict) for rule in rules):
        raise SpecError("stop must be an array of tables")
    for rule in rules:
        if ("when" in rule) == ("hook" in rule):
            raise SpecError("each stop rule needs exactly one of when or hook")
        if "when" in rule:
            expression = Expression(rule["when"], spec["kind"] == "ab")
            rule.setdefault("scope", "any")
            rule.setdefault("action", "abort")
            rule.setdefault("after_minute", 0)
            if rule["scope"] not in ("any", "all") or rule["action"] not in ("abort", "finish"):
                raise SpecError("stop scope must be any/all and action abort/finish")
            threshold = rule["after_minute"]
            if type(threshold) not in (int, float) or not math.isfinite(threshold) or threshold < 0:
                raise SpecError("after_minute must be a nonnegative number")
            if "arms" in rule:
                if spec["kind"] != "ab" or not isinstance(rule["arms"], list) or not rule["arms"] \
                        or any(name not in spec["arms"] for name in rule["arms"]):
                    raise SpecError("stop arms must select existing ab arms")
            if any(name not in spec.get("arms", {}) for name, _ in expression.peers):
                raise SpecError("arm() names an unknown arm")
        else:
            rule["hook"] = str(ctx.path(spec, rule["hook"]).resolve(strict=True))
            rule.setdefault("every_min", 10)
            interval = rule["every_min"]
            if type(interval) not in (int, float) or not math.isfinite(interval) or interval <= 0:
                raise SpecError("every_min must be a positive number")
        if "reason" in rule and not isinstance(rule["reason"], str):
            raise SpecError("stop reason must be a string")
    # An explicit output/resume directory can already contain known census headers.
    if any("when" in rule for rule in rules) and (spec.get("out") or spec.get("resume") or spec.get("outputs")):
        from .core import Job
        job = Job(dict(id="validation", kind=spec["kind"], name=None, state="submitted", spec=json.dumps(spec),
                       kstate="{}", out=spec.get("out", "."), reason=None), [])
        evaluate(job, kind.census_runs(job, ctx), headers_only=True)
    return spec


def evaluate(job, runs, headers_only=False):
    rules = [r for r in job.spec.get("stop", []) if "when" in r]
    if not rules:
        return None
    data = {(r.arm, str(r.seed)): census(r) for r in runs}
    for index, rule in enumerate(job.spec["stop"]):
        if "when" not in rule:
            continue
        expression = Expression(rule["when"], job.kind == "ab")
        selected = [r for r in runs if "arms" not in rule or r.arm in rule["arms"]]
        matches = []
        for run in selected:
            headers, history = data[run.arm, str(run.seed)]
            for header in headers:
                expression.headers(header, expression.columns)
            peers = {}
            for name, column in expression.peers:
                other_headers, other = data.get((name, str(run.seed)), ([], []))
                for header in other_headers:
                    expression.headers(header, [column])
                peers[name] = other
            if headers_only:
                continue
            if not history or sim_minute(history[-1]) < rule["after_minute"]:
                matches.append(False)
                continue
            try:
                matches.append(expression.value(history, peers))
            except Waiting:
                matches.append(False)
        if matches and (any(matches) if rule["scope"] == "any" else all(matches)):
            return dict(rule=index, action=rule["action"], reason=rule.get("reason") or rule["when"])
    return None


def run_hook(rule, out):
    reply = subprocess.run([rule["hook"], str(out)], capture_output=True, text=True, timeout=60,
                           stdin=subprocess.DEVNULL, check=True)
    lines = reply.stdout.strip().splitlines()
    if len(lines) != 1:
        raise ValueError("stop hook must print one JSON line")
    result = json.loads(lines[0])
    if not isinstance(result, dict) or type(result.get("stop")) is not bool \
            or result.get("action") not in ("abort", "finish") or not isinstance(result.get("reason"), str):
        raise ValueError("stop hook needs stop: bool, action: abort/finish, reason: string")
    return result if result["stop"] else None


def rule_labels(spec):
    for rule in spec.get("stop", []):
        if "when" in rule:
            arms = ",".join(rule["arms"]) if rule.get("arms") else "all arms"
            yield (f"stop {rule.get('scope', 'any')} {arms} after {rule.get('after_minute', 0)}m: "
                   f"{rule['when']} -> {rule.get('action', 'abort')} ({rule.get('reason', '')})")
        else:
            yield f"stop hook {rule['hook']} every {rule.get('every_min', 10)} wall min"
