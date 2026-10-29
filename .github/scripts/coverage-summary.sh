#!/usr/bin/env bash
# Turns the JSON of `cargo llvm-cov report --json --summary-only` into a
# Markdown summary: line, function and region coverage per crate and in total.
#
# Usage: coverage-summary.sh coverage.json > coverage.md
set -euo pipefail

jq -r '
  def pct(c; n): if n == 0 then "-" else "\(c * 10000 / n | round / 100)%" end;
  def row(name; s):
    "| \(name) | \(pct(s.lines.covered; s.lines.count)) (\(s.lines.covered)/\(s.lines.count))"
    + " | \(pct(s.functions.covered; s.functions.count))"
    + " | \(pct(s.regions.covered; s.regions.count)) |";
  def add_up: reduce .[] as $s (
    {lines: {count: 0, covered: 0}, functions: {count: 0, covered: 0}, regions: {count: 0, covered: 0}};
    .lines.count += $s.lines.count | .lines.covered += $s.lines.covered
    | .functions.count += $s.functions.count | .functions.covered += $s.functions.covered
    | .regions.count += $s.regions.count | .regions.covered += $s.regions.covered);

  .data[0] as $d
  | "## Test coverage",
    "",
    "| Crate | Lines | Functions | Regions |",
    "|---|---:|---:|---:|",
    ($d.files
      | group_by(.filename | capture("crates/(?<crate>[^/]+)/").crate)
      | .[]
      | row("`\(.[0].filename | capture("crates/(?<crate>[^/]+)/").crate)`"; map(.summary) | add_up)),
    row("**Total**"; $d.totals)
' "$1"
