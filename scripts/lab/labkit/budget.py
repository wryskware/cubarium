"""EC2 budget: the dollars Wrysk granted, what instances spent and what running approvals
may still spend. An approval whose --max-usd exceeds what is left is refused before anything
launches. The budget only ever blocks launches: it never stops or alters a running instance.

Tables (store.SCHEMA; CREATE IF NOT EXISTS, so an old database gains them in place on open):
  ec2_grants  one row per authorization: when, USD and his words (a note is required)
  ec2_spend   one row per terminated instance: hours and estimated USD at its type's price

Spent = ec2_spend rows + the running instance's cost so far. On first open of a database that
predates the table, `migrate` copies every past `host-down` (host ec2, with est_usd) from the
event log once, keyed by event seq, so a second run adds nothing. Prices are the configured
spot prices: estimates; the AWS bill is the truth.
"""

import datetime
import json
import math

from .store import read_events

BACKFILLED = "budget:backfilled"
LIVE = ("launching", "up", "down-failed")  # an instance exists and costs money
EPS = 1e-9


class BudgetError(ValueError):
    """An approval over the remaining budget; more budget needs Wrysk's word."""


def past_downs(events_path):
    """ec2 `host-down` events that carry a cost estimate, in log order."""
    records, _ = read_events(events_path)
    return [r for r in records if r.get("event") == "host-down" and r.get("host") == "ec2"
            and r.get("est_usd") is not None]


def migrate(store, events_path):
    """Count the event log's past ec2 spend once (idempotent: keyed by seq, then marked)."""
    if store.kv_get(BACKFILLED):
        return 0
    added = 0
    for record in past_downs(events_path):
        before = store.one("SELECT 1 FROM ec2_spend WHERE key=?", f"event:{record['seq']}")
        store.x("INSERT OR IGNORE INTO ec2_spend(key, instance, approval, ended, hours, usd, reason) "
                "VALUES(?, ?, ?, ?, ?, ?, ?)", f"event:{record['seq']}", record.get("instance"),
                record.get("approval"), record.get("t"), record.get("hours"), float(record["est_usd"]),
                record.get("reason"))
        added += before is None
    store.kv_set(BACKFILLED, dict(rows=added))
    return added


def record_spend(store, instance, approval, ended, hours, usd, reason):
    key = f"{instance or 'unknown'}@{ended:.3f}"
    store.x("INSERT OR IGNORE INTO ec2_spend(key, instance, approval, ended, hours, usd, reason) "
            "VALUES(?, ?, ?, ?, ?, ?, ?)", key, instance, approval, ended, hours, usd, reason)


def check_grant(usd, note):
    if not (isinstance(usd, (int, float)) and math.isfinite(usd) and usd > 0):
        raise ValueError("a budget grant needs a positive USD amount")
    if not (note or "").strip():
        raise ValueError("a budget grant needs --note with Wrysk's words authorizing it")


def grant(store, usd, note, at):
    check_grant(usd, note)
    store.x("INSERT INTO ec2_grants(at, usd, note) VALUES(?, ?, ?)", float(at), float(usd), note.strip())


def parse_at(text, now):
    """--at accepts an ISO date or date-time (local time); default now."""
    if not text:
        return now
    return datetime.datetime.fromisoformat(text).timestamp()


def _has(store, table):
    return store.one("SELECT 1 FROM sqlite_master WHERE type='table' AND name=?", table) is not None


def summary(store, ec2_state, price_of, now, events_path=None):
    """Granted, spent (finished + running so far), committed and remaining, in USD.

    price_of(type) -> spot USD/h or None. A read-only store that predates the tables reads
    past spend from the event log instead (what `migrate` will copy on the daemon's start).
    """
    if _has(store, "ec2_grants"):
        grants = [dict(r) for r in store.q("SELECT at, usd, note FROM ec2_grants ORDER BY at, id")]
    else:
        grants = []
    if _has(store, "ec2_spend") and store.kv_get(BACKFILLED):
        finished = [dict(r) for r in store.q("SELECT * FROM ec2_spend ORDER BY ended")]
    else:  # not migrated yet: the same rows migrate() would write
        finished = [dict(instance=r.get("instance"), approval=r.get("approval"), ended=r.get("t"),
                         hours=r.get("hours"), usd=float(r["est_usd"]), reason=r.get("reason"))
                    for r in (past_downs(events_path) if events_path else [])]
    s = ec2_state or {}
    running = None
    committed = 0.0
    if s.get("state") in LIVE and s.get("launched") is not None:
        price = s.get("price_usd_h")
        if price is None:
            price = price_of(s.get("type"))
        hours = max(0.0, (now - s["launched"]) / 3600)
        usd = hours * float(price or 0)
        running = dict(approval=s.get("approval"), type=s.get("type"), hours=hours, usd=usd,
                       price_known=price is not None, max_usd=s.get("max_usd"))
        committed = max(0.0, float(s.get("max_usd") or 0) - usd)
    elif s.get("state") == "approved":  # granted, launch in progress: the whole cap is held
        committed = float(s.get("max_usd") or 0)
    granted = sum(g["usd"] for g in grants)
    spent_done = sum(f["usd"] for f in finished)
    spent = spent_done + (running["usd"] if running else 0.0)
    return dict(granted=granted, spent=spent, spent_finished=spent_done, finished=finished,
                running=running, committed=committed, remaining=granted - spent - committed,
                grants=grants)


def check(budget, max_usd):
    """Refuse an approval over the remaining budget (raises BudgetError)."""
    if max_usd > budget["remaining"] + EPS:
        raise BudgetError(
            f"EC2 budget: ${max(0.0, budget['remaining']):.2f} left of ${budget['granted']:.2f} granted "
            f"(spent ${budget['spent']:.2f}, committed ${budget['committed']:.2f}); --max-usd {max_usd:g} "
            f"exceeds it. More budget needs Wrysk's word, recorded with "
            f"`lab budget grant USD --note \"<his words>\"`.")


def status_line(budget):
    return f"ec2 budget: ${budget['remaining']:.2f} left of ${budget['granted']:.2f}"


def report(budget):
    """The `lab budget` text."""
    lines = ["EC2 budget (estimates at configured spot prices; the AWS bill is the truth)"]
    finished = budget["finished"]
    running = budget["running"]
    spent_note = f"{len(finished)} finished ${budget['spent_finished']:.2f}"
    if running:
        price = "" if running["price_known"] else ", price unknown"
        spent_note += f" + running {running['type']} {running['hours']:.2f} h ${running['usd']:.2f}{price}"
    lines.append(f"  granted    ${budget['granted']:8.2f}")
    lines.append(f"  spent      ${budget['spent']:8.2f}   ({spent_note})")
    commit_note = ""
    if running:
        commit_note = (f"   (approval {running['approval']}: max ${float(running['max_usd'] or 0):.2f} "
                       f"less ${running['usd']:.2f} spent)")
    lines.append(f"  committed  ${budget['committed']:8.2f}{commit_note}")
    lines.append(f"  remaining  ${budget['remaining']:8.2f}")
    lines.append("grants:")
    if not budget["grants"]:
        lines.append("  none: approvals are refused until Wrysk grants a budget")
    for g in budget["grants"]:
        when = datetime.datetime.fromtimestamp(g["at"]).strftime("%Y-%m-%d %H:%M")
        lines.append(f"  {when}  ${g['usd']:.2f}  {json.dumps(g['note'], ensure_ascii=False)}")
    if finished:
        lines.append("finished instances:")
        for f in finished:
            when = datetime.datetime.fromtimestamp(f["ended"]).strftime("%Y-%m-%d %H:%M") if f.get("ended") else "?"
            hours = f"{f['hours']:.2f} h" if f.get("hours") is not None else "? h"
            lines.append(f"  {when}  {f.get('instance') or '?'}  {hours}  ${f['usd']:.2f}  {f.get('reason') or ''}")
    return "\n".join(lines)
