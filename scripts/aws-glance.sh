#!/usr/bin/env bash
# Read-only glance at the cubarium AWS training hosts, for a desktop bar widget.
# One `aws ec2 describe-instances`, then one ssh per running host (in parallel) reading
# /proc/loadavg and nproc. Prints one line of JSON:
#   {"ok":true,"error":"","hosts":[{"id","type","market","up_s","load_pct","ncpu"}...]}
# load_pct = 1-minute loadavg / nproc * 100, capped at 100; null when that host's ssh failed.
# ok=false (hosts empty) when the AWS call failed. Never starts, stops, ships or modifies anything.
# Same discovery and ssh settings as scripts/aws-graviton.sh (tag project=cubarium-train).
set -u
REGION="${AWS_REGION:-us-east-1}"
TAG=cubarium-train
KEY_FILE="$HOME/.ssh/cubarium-aws.pem"
ssh_opts=(-i "$KEY_FILE" -o BatchMode=yes -o StrictHostKeyChecking=accept-new -o LogLevel=ERROR -o ConnectTimeout=5)

tmp=$(mktemp -d) && trap 'rm -rf "$tmp"' EXIT

if ! AWS_PAGER="" timeout 25 aws --region "$REGION" ec2 describe-instances \
	--filters "Name=tag:project,Values=$TAG" Name=instance-state-name,Values=running \
	--query 'Reservations[].Instances[].[InstanceId,PublicIpAddress,InstanceLifecycle,LaunchTime,InstanceType]' \
	--output text >"$tmp/hosts" 2>"$tmp/err"; then
	python3 -c 'import json,sys; print(json.dumps({"ok":False,"error":sys.stdin.read().strip()[:200] or "aws failed","hosts":[]}))' <"$tmp/err"
	exit 0
fi

n=0
while IFS=$'\t' read -r id ip life launch type; do
	[ -n "$id" ] || continue
	n=$((n + 1))
	printf '%s\t%s\t%s\t%s\t%s\n' "$id" "$ip" "$life" "$launch" "$type" >"$tmp/h.$n"
	(
		[ "$ip" != None ] && timeout 12 ssh "${ssh_opts[@]}" "ec2-user@$ip" 'cat /proc/loadavg; nproc' \
			>"$tmp/l.$n" 2>/dev/null || : >"$tmp/l.$n"
	) &
done <"$tmp/hosts"
wait

python3 - "$tmp" "$n" <<'PY'
import json, sys, time
from datetime import datetime
tmp, n = sys.argv[1], int(sys.argv[2])
hosts = []
for i in range(1, n + 1):
    id_, ip, life, launch, type_ = open(f"{tmp}/h.{i}").read().rstrip("\n").split("\t")
    up = max(0, int(time.time() - datetime.fromisoformat(launch).timestamp()))
    load = ncpu = None
    try:
        a, b = open(f"{tmp}/l.{i}").read().split("\n")[:2]
        ncpu = int(b)
        load = min(100, round(float(a.split()[0]) / ncpu * 100))
    except Exception:
        pass
    hosts.append({"id": id_, "type": type_, "market": "spot" if life == "spot" else "on-demand",
                  "up_s": up, "load_pct": load, "ncpu": ncpu})
hosts.sort(key=lambda h: -h["up_s"])
print(json.dumps({"ok": True, "error": "", "hosts": hosts}, separators=(",", ":")))
PY
