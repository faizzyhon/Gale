#!/usr/bin/env bash
# Gale vs aria2c vs curl on a local test server. Every file is checked against a reference SHA-256.
# Needs: cargo build --release -p gale-core --example bench_server --example bench_get; aria2c; curl; bc.
# Results go to $B/results.csv (printed as it runs). Files are written to a temporary folder and deleted.
set -u
ROOT=$(cd "$(dirname "$0")/../.." && pwd); B=$(mktemp -d); T=$ROOT/target/release/examples
$T/bench_server 8900 & SRV=$!; sleep 2
H=http://127.0.0.1:8900
ref() { curl -s "$H/$1/fast/file.bin" | sha256sum | cut -d' ' -f1; }
run() { # scenario tool size mode
  local s=$1 tool=$2 mib=$3 mode=$4 out=$B/out.bin; rm -f $out $out.aria2 $B/out.bin.gale-part
  local url="$H/$mib/$mode/file.bin" t0 t1 ok code
  t0=$(date +%s.%N)
  case $tool in
    gale) $T/bench_get "$url" $out 16 >/dev/null 2>$B/err.txt; code=$? ;;
    aria2c) aria2c -q -x16 -s16 -k1M --file-allocation=none --max-tries=0 --retry-wait=1 --allow-overwrite=true -d $B -o out.bin "$url" 2>$B/err.txt; code=$? ;;
    curl) timeout 600 curl -sS --retry 30 --retry-all-errors --retry-delay 1 -C - -o $out "$url" 2>$B/err.txt; code=$? ;;
  esac
  t1=$(date +%s.%N)
  local secs=$(echo "$t1 - $t0" | bc -l)
  if [ $code -eq 0 ] && [ "$(sha256sum $out | cut -d' ' -f1)" = "${REF[$mib]}" ]; then ok=ok; else ok="fail(exit $code: $(head -c 120 $B/err.txt | tr '\n' ' '))"; fi
  printf "%s,%s,%s,%s,%.1f,%.1f,%s\n" "$s" "$tool" "$mib" "$mode" "$secs" "$(echo "$mib / $secs" | bc -l)" "$ok" | tee -a $B/results.csv
  rm -f $out $out.aria2
}
declare -A REF; for m in 128 256 1024; do REF[$m]=$(ref $m); done
echo "scenario,tool,mib,mode,seconds,mib_per_s,result" > $B/results.csv
for tool in gale aria2c curl; do run A $tool 256 throttle; done
for tool in gale aria2c curl; do run B $tool 1024 fast; done
for tool in gale aria2c curl; do run C $tool 128 limit4; done
for tool in gale aria2c curl; do run D $tool 256 flaky; done
kill $SRV
cat $B/results.csv; rm -rf $B
