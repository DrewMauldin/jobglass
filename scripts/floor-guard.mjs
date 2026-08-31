import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const roots = [
  "src",
  "src-tauri/src",
  "src-tauri/tests",
  "src-tauri/benches",
  "tests",
  ".github",
  "scripts",
];
const extensions = new Set([
  ".js",
  ".mjs",
  ".rs",
  ".sh",
  ".ts",
  ".tsx",
  ".yml",
  ".yaml",
]);
const forbidden = [
  /@ts-ignore/u,
  /eslint-disable/u,
  /#!\s*\[allow\s*\(/u,
  /(?:describe|it|test)\.skip\s*\(/u,
  /todo!\s*\(/u,
  /unimplemented!\s*\(/u,
];

async function filesBelow(root) {
  const entries = await readdir(root, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map(async (entry) => {
      const candidate = path.join(root, entry.name);
      return entry.isDirectory() ? filesBelow(candidate) : [candidate];
    }),
  );
  return nested.flat();
}

const files = [];
for (const root of roots) {
  try {
    files.push(...(await filesBelow(root)));
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      continue;
    }
    throw error;
  }
}

const failures = [];
const guardPath = path.normalize("scripts/floor-guard.mjs");
for (const file of files.filter(
  (candidate) =>
    candidate !== guardPath && extensions.has(path.extname(candidate)),
)) {
  const contents = await readFile(file, "utf8");
  for (const pattern of forbidden) {
    if (pattern.test(contents)) failures.push(`${file}: ${String(pattern)}`);
  }
}

if (failures.length > 0) {
  console.error(failures.join("\n"));
  process.exitCode = 1;
} else {
  console.log(
    `floor guard: ${String(files.length)} files inspected, zero suppressions or stubs`,
  );
}
