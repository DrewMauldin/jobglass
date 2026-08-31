import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";

const threshold = 80;
const base =
  process.argv[2] ??
  (process.env.GITHUB_BASE_REF
    ? `origin/${process.env.GITHUB_BASE_REF}`
    : "origin/main");
const diff = execFileSync(
  "git",
  ["diff", "--unified=0", "--no-color", `${base}...HEAD`, "--", "src"],
  { encoding: "utf8" },
);

const changed = new Map();
let currentFile;
for (const line of diff.split("\n")) {
  if (line.startsWith("+++ b/")) {
    const candidate = line.slice(6);
    currentFile =
      /\.test\.[cm]?[jt]sx?$/u.test(candidate) ||
      candidate.startsWith("src/test/") ||
      candidate === "src/types.ts" ||
      candidate.endsWith(".css")
        ? undefined
        : candidate;
    continue;
  }
  const hunk = line.match(/^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@/u);
  if (!hunk || !currentFile) continue;
  const start = Number(hunk[1]);
  const count = Number(hunk[2] ?? "1");
  const lines = changed.get(currentFile) ?? new Set();
  for (let offset = 0; offset < count; offset += 1) lines.add(start + offset);
  changed.set(currentFile, lines);
}

const coverage = new Map();
let source;
for (const line of (await readFile("coverage/lcov.info", "utf8")).split("\n")) {
  if (line.startsWith("SF:")) {
    source = line.slice(3).replaceAll("\\", "/");
    coverage.set(source, new Map());
    continue;
  }
  const data = line.match(/^DA:(\d+),(\d+)/u);
  if (data && source)
    coverage.get(source).set(Number(data[1]), Number(data[2]));
}

let executable = 0;
let covered = 0;
const missed = [];
const missingFiles = [];
for (const [file, lines] of changed) {
  const fileCoverage = coverage.get(file);
  if (!fileCoverage) {
    missingFiles.push(file);
    continue;
  }
  for (const line of lines) {
    const hits = fileCoverage.get(line);
    if (hits === undefined) continue;
    executable += 1;
    if (hits > 0) covered += 1;
    else missed.push(`${file}:${line}`);
  }
}

if (missingFiles.length > 0) {
  console.error(
    `changed source files missing from coverage: ${missingFiles.join(", ")}`,
  );
  process.exit(1);
}

if (executable === 0) {
  console.log(
    `changed-line coverage: no executable frontend lines since ${base}`,
  );
  process.exit(0);
}
const percentage = (covered / executable) * 100;
console.log(
  `changed-line coverage: ${covered}/${executable} (${percentage.toFixed(1)}%) since ${base}`,
);
if (percentage < threshold) {
  console.error(`uncovered changed lines: ${missed.join(", ")}`);
  process.exit(1);
}
