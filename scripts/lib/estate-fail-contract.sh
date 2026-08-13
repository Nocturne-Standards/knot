estate_gate_line() {
  # $1 name $2 status $3 severity
  echo "GATE:$1 STATUS:$2 SEVERITY:$3" >&2
}
estate_hint() {
  echo "HINT: $*" >&2
}
