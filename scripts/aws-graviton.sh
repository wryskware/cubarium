#!/bin/bash
# Graviton offload hosts for training (mode A: the whole trainer runs on the AWS host).
# Runbook and cost matrix: docs/graviton-training.md. Stateless: hosts are found by tag.
#
#   up [-n COUNT] [-t TYPES] [--ondemand] [--hours H] [--dry-run]
#        launch COUNT hosts. TYPES is a comma list tried in order (default
#        c7g.16xlarge,c6g.16xlarge): the first type with capacity wins.
#   ship                 build natively on the first host from the working tree (tracked files,
#                        uncommitted edits included), save the binary to
#                        target/aarch64-graviton2/, push it to every host
#   put FILE...          copy files (repo-relative paths, e.g. a warm-start centre under runs/)
#                        to ~/cubarium/ on every host
#   run                  retired: use scripts/graviton-studies.py run (bounded study kit)
#   logs N NAME [LINES]  tail that log
#   fetch N              pull ~/cubarium/runs/ from host N into runs/aws-<ip>/
#   status               hosts: n, id, ip, state, market, launch time, type
#   ssh N [cmd]          ssh to host N (row number of `status`)
#   down                 terminate every tagged host (no data lives there; fetch first)
#
# Cost guards: spot by default; each host powers itself off after --hours (default 12) and
# instance-initiated shutdown TERMINATES it; EBS is delete-on-terminate. ssh is admitted from
# this machine's current public IP only; no IAM role on the hosts.
set -euo pipefail
cd "$(dirname "$0")/.."

REGION="${AWS_REGION:-us-east-1}"
TAG=cubarium-train
KEY_NAME=cubarium-aws
KEY_FILE="$HOME/.ssh/cubarium-aws.pem"
SG_NAME=cubarium-train-ssh
OUT=target/aarch64-graviton2
aws_() { aws --region "$REGION" "$@"; }
ssh_opts=(-i "$KEY_FILE" -o BatchMode=yes -o StrictHostKeyChecking=accept-new -o LogLevel=ERROR)

hosts() { # id, ip, state, market, launch time, type
	aws_ ec2 describe-instances \
		--filters "Name=tag:project,Values=$TAG" Name=instance-state-name,Values=pending,running \
		--query 'Reservations[].Instances[].[InstanceId,PublicIpAddress,State.Name,InstanceLifecycle,LaunchTime,InstanceType]' \
		--output text | sort -k5
}
all_host_ids() { # down must also find stopped/stopping instances, not just running ones.
	aws_ ec2 describe-instances \
		--filters "Name=tag:project,Values=$TAG" Name=instance-state-name,Values=pending,running,stopping,stopped \
		--query 'Reservations[].Instances[].InstanceId' --output text
}
ips() { hosts | awk '$3=="running" && $2!="None" {print $2}'; }
host_ip() {
	local ip
	ip=$(ips | sed -n "${1}p")
	[ -n "$ip" ] || { echo "no running host $1 (see: $0 status)" >&2; exit 1; }
	echo "$ip"
}

ensure_key() {
	local err tmp
	# Create the pair only when AWS says it is absent: any other failure (an expired login) must
	# not touch the key file, or the private half of a live pair is lost.
	if ! err=$(aws_ ec2 describe-key-pairs --key-names "$KEY_NAME" 2>&1 >/dev/null); then
		case "$err" in
		*InvalidKeyPair.NotFound*) ;;
		*) echo "cannot check key pair $KEY_NAME: $err" >&2; exit 1 ;;
		esac
		[ -s "$KEY_FILE" ] && { echo "$KEY_FILE exists but AWS has no pair $KEY_NAME; move the file aside and rerun" >&2; exit 1; }
		mkdir -p "$(dirname "$KEY_FILE")"
		tmp=$(mktemp "$KEY_FILE.XXXX")
		aws_ ec2 create-key-pair --key-name "$KEY_NAME" --query KeyMaterial --output text >"$tmp" ||
			{ rm -f "$tmp"; exit 1; }
		chmod 600 "$tmp"
		mv "$tmp" "$KEY_FILE"
	fi
	[ -s "$KEY_FILE" ] || { echo "key pair $KEY_NAME exists in AWS but $KEY_FILE is missing or empty; delete the AWS key pair and rerun" >&2; exit 1; }
}

ensure_sg() {
	local vpc sg me
	vpc=$(aws_ ec2 describe-vpcs --filters Name=is-default,Values=true --query 'Vpcs[0].VpcId' --output text)
	sg=$(aws_ ec2 describe-security-groups --filters "Name=group-name,Values=$SG_NAME" "Name=vpc-id,Values=$vpc" \
		--query 'SecurityGroups[0].GroupId' --output text)
	if [ "$sg" = None ]; then
		sg=$(aws_ ec2 create-security-group --group-name "$SG_NAME" --vpc-id "$vpc" \
			--description "cubarium training hosts: ssh from one IP" --query GroupId --output text)
	fi
	me=$(curl -fsS https://checkip.amazonaws.com | tr -d '[:space:]')
	aws_ ec2 authorize-security-group-ingress --group-id "$sg" --protocol tcp --port 22 --cidr "$me/32" >/dev/null 2>&1 || true
	echo "$sg"
}

cmd_up() {
	local count=1 types=c7g.16xlarge,c6g.16xlarge market=spot hours=12 dry=()
	while [ $# -gt 0 ]; do
		case "$1" in
		-n) count=$2; shift 2 ;;
		-t) types=$2; shift 2 ;;
		--ondemand) market=ondemand; shift ;;
		--dry-run) dry=(--dry-run); shift ;;
		--hours) hours=$2; shift 2 ;;
		*) echo "unknown option $1" >&2; exit 2 ;;
		esac
	done
	ensure_key
	local sg ami ud type out
	sg=$(ensure_sg)
	ami=$(aws_ ssm get-parameter --name /aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64 \
		--query Parameter.Value --output text)
	ud=$(mktemp)
	cat >"$ud" <<EOF
#!/bin/bash
set -euo pipefail
shutdown -h +$((hours * 60))
dnf install -y gcc gcc-c++ git tar make pkgconf-pkg-config python3.11 rsync util-linux > /var/log/bootstrap.log 2>&1
mkdir -p /home/ec2-user/cubarium-train && chown ec2-user /home/ec2-user/cubarium-train
touch /var/tmp/ready
EOF
	for type in ${types//,/ }; do
		local args=(--image-id "$ami" --instance-type "$type" --count "$count" --key-name "$KEY_NAME"
			--security-group-ids "$sg" --instance-initiated-shutdown-behavior terminate
			--block-device-mappings 'DeviceName=/dev/xvda,Ebs={VolumeSize=40,VolumeType=gp3,DeleteOnTermination=true}'
			--user-data "file://$ud"
			--tag-specifications "ResourceType=instance,Tags=[{Key=project,Value=$TAG},{Key=Name,Value=$TAG}]")
		[ "$market" = spot ] && args+=(--instance-market-options 'MarketType=spot,SpotOptions={SpotInstanceType=one-time,InstanceInterruptionBehavior=terminate}')
		echo "trying $count x $type ($market)..." >&2
		if out=$(aws_ ec2 run-instances "${dry[@]}" "${args[@]}" --query 'Instances[].InstanceId' --output text 2>&1); then
			echo "$out"
			echo "launched $type; auto-terminates in ${hours}h. Next: \`$0 status\`, then \`$0 ship\`." >&2
			rm -f "$ud"
			return 0
		fi
		if [[ "$out" == *DryRunOperation* ]]; then
			echo "dry run ok for $type" >&2
			rm -f "$ud"
			return 0
		fi
		echo "  $type failed: $(echo "$out" | tail -1 | cut -c1-160)" >&2
	done
	rm -f "$ud"
	echo "no type in [$types] could be launched" >&2
	return 1
}

wait_ready() {
	local ip
	for ip in $(ips); do
		ssh "${ssh_opts[@]}" -o ConnectTimeout=5 "ec2-user@$ip" 'test -f /var/tmp/ready' ||
			{ echo "$ip not ready yet; retry ship once bootstrap is complete" >&2; return 1; }
	done
}

# Tracked files as they are on disk now (uncommitted edits ride along), minus bulk dirs.
src_tar() {
	git ls-files -z -- . ':(exclude)art' ':(exclude)design' ':(exclude)captures' ':(exclude)runs' \
		':(exclude)graft' ':(exclude)docs' ':(exclude)spikes' ':(exclude)assets/atelier' ':(exclude)assets/voxel-textures' |
		tar --null -T - -c --ignore-failed-read
}

cmd_ship() {
	local first ip
	wait_ready
	first=$(ips | head -1)
	[ -n "$first" ] || { echo "no running hosts" >&2; exit 1; }
	src_tar | ssh "${ssh_opts[@]}" "ec2-user@$first" 'mkdir -p ~/cubarium && tar -x -C ~/cubarium'
	ssh "${ssh_opts[@]}" "ec2-user@$first" 'bash -s' <<'EOF'
set -e
command -v ~/.cargo/bin/cargo >/dev/null || curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.98.1 >/dev/null
. ~/.cargo/env
cd ~/cubarium
cargo build --release -p cubarium-search -j "$(nproc)"
strip -o ~/cubarium-train/cubarium-search.new target/release/cubarium-search
mv ~/cubarium-train/cubarium-search.new ~/cubarium-train/cubarium-search
EOF
	mkdir -p "$OUT"
	scp -q "${ssh_opts[@]}" "ec2-user@$first:cubarium-train/cubarium-search" "$OUT/cubarium-search"
	echo "built on $first from $(git rev-parse --short HEAD)$(git diff --quiet HEAD -- || echo '+uncommitted'); saved $OUT/cubarium-search"
	for ip in $(ips | tail -n +2); do
		# The source tree goes too, so every host can run from ~/cubarium.
		src_tar | ssh "${ssh_opts[@]}" "ec2-user@$ip" 'mkdir -p ~/cubarium ~/cubarium-train && tar -x -C ~/cubarium'
		scp -q "${ssh_opts[@]}" "$OUT/cubarium-search" "ec2-user@$ip:cubarium-train/cubarium-search"
		echo "shipped to $ip"
	done
	# Runs use the same binary path on every host.
	for ip in $(ips); do
		ssh "${ssh_opts[@]}" "ec2-user@$ip" 'mkdir -p ~/cubarium/target/release ~/cubarium/runs && cp ~/cubarium-train/cubarium-search ~/cubarium/target/release/cubarium-search'
	done
}

cmd_put() {
	local ip f
	[ $# -gt 0 ] || { echo "usage: put FILE..." >&2; exit 2; }
	for ip in $(ips); do
		tar -c "$@" | ssh "${ssh_opts[@]}" "ec2-user@$ip" 'mkdir -p ~/cubarium && tar -x -C ~/cubarium'
		echo "put $# path(s) on $ip"
	done
}

cmd_run() {
	echo "legacy run retired: use scripts/graviton-studies.py run --spec JOBS.json --host ec2-user@IP (see docs/graviton-training.md)" >&2
	return 2
}

cmd_logs() {
	ssh "${ssh_opts[@]}" "ec2-user@$(host_ip "$1")" "tail -n ${3:-40} ~/cubarium/runs/$2.log"
}

cmd_fetch() {
	local ip dest
	ip=$(host_ip "$1")
	dest="runs/aws-$ip"
	mkdir -p "$dest"
	ssh "${ssh_opts[@]}" "ec2-user@$ip" 'cd ~/cubarium/runs && tar -c .' | tar -x -C "$dest"
	echo "fetched into $dest"
}

cmd_status() { hosts | nl -w1 -s' '; }

cmd_ssh() {
	local ip
	ip=$(host_ip "$1")
	shift
	exec ssh "${ssh_opts[@]}" "ec2-user@$ip" "$@"
}

cmd_down() {
	local ids
	ids=$(all_host_ids)
	[ -n "$ids" ] || { echo "nothing running"; return; }
	# shellcheck disable=SC2086
	aws_ ec2 terminate-instances --instance-ids $ids --query 'TerminatingInstances[].[InstanceId,CurrentState.Name]' --output text
}

case "${1:-}" in
up) shift; cmd_up "$@" ;;
ship) cmd_ship ;;
put) shift; cmd_put "$@" ;;
run) shift; cmd_run "$@" ;;
logs) shift; cmd_logs "$@" ;;
fetch) shift; cmd_fetch "$@" ;;
status) cmd_status ;;
ssh) shift; cmd_ssh "$@" ;;
down) cmd_down ;;
*) sed -n '2,26p' "$0"; exit 2 ;;
esac
