#!/usr/bin/env bash

set -euo pipefail

if [[ $# -lt 1 ]]; then
    printf "Usage: %s <pattern>\n" "$(basename "$0")" >&2
    exit 1
fi

pattern="$1"

printf "%-8s %-4s %-12s %-24s\n" "PID" "NI" "AFFINITY" "COMM"
printf "%-8s %-4s %-12s %-24s\n" "--------" "----" "------------" "------------------------"

ps -eo pid=,ni=,comm= \
    | grep -i -- "$pattern" \
    | while read -r pid ni comm; do
        affinity="$(taskset -pc "$pid" 2>/dev/null | sed 's/.*: //')"
        if [[ -z "$affinity" ]]; then
            affinity="?"
        fi
        printf "%-8s %-4s %-12s %-24s\n" "$pid" "$ni" "$affinity" "$comm"
    done
