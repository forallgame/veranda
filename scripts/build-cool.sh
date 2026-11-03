#!/usr/bin/env bash
# Run a cargo command with a thermal watchdog for laptops that overheat.
# The compiler is paused (SIGSTOP) when the CPU package passes $HOT and resumed (SIGCONT)
# once it cools below $COOL. One job, lowest priority.
#
#   scripts/build-cool.sh build -p veranda-desktop
#   HOT=78 COOL=62 scripts/build-cool.sh build --release -p veranda-desktop
set -u
HOT=${HOT:-80}; COOL=${COOL:-65}; JOBS=${JOBS:-1}
zone=$(grep -l x86_pkg_temp /sys/class/thermal/thermal_zone*/type 2>/dev/null | head -1 | xargs -r dirname)
temp() { [ -n "$zone" ] && echo $(( $(cat "$zone/temp") / 1000 )) || echo 0; }
until [ "$(temp)" -le "$COOL" ]; do sleep 5; done
# put -j before any `--` so it reaches cargo, not rustc
sub="$1"; shift
nice -n 19 cargo "$sub" -j "$JOBS" "$@" &
pid=$!
paused=0
while kill -0 "$pid" 2>/dev/null; do
  t=$(temp)
  if [ "$paused" -eq 0 ] && [ "$t" -ge "$HOT" ]; then pkill -STOP -P "$pid"; kill -STOP "$pid"; paused=1; echo "[cool] paused at ${t}C"; fi
  if [ "$paused" -eq 1 ] && [ "$t" -le "$COOL" ]; then kill -CONT "$pid"; pkill -CONT -P "$pid"; paused=0; echo "[cool] resumed at ${t}C"; fi
  sleep 2
done
wait "$pid"
