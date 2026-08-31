import { existsSync, readdirSync, statSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { dirname, extname, join, resolve } from "node:path";

const root = process.cwd();
const roots = [
  "README.md",
  "CHANGELOG.md",
  "CODE_OF_CONDUCT.md",
  "CONTRIBUTING.md",
  "ROADMAP.md",
  "SECURITY.md",
  "SPEC.md",
  "SUPPORT_MATRIX.md",
  "CONSTRAINTS.md",
  "CAPABILITY_MAP.md",
  "docs",
  "site",
];

function walk(path) {
  const absolute = resolve(root, path);
  if (!existsSync(absolute)) return [];
  if (!statSync(absolute).isDirectory()) return [absolute];
  return readdirSync(absolute, { withFileTypes: true }).flatMap((entry) =>
    walk(join(path, entry.name)),
  );
}

function localTargets(contents, extension) {
  const matches = [];
  if (extension === ".md") {
    for (const match of contents.matchAll(
      /!?\[[^\]]*\]\(([^\s)]+)(?:\s+"[^"]*")?\)/g,
    )) {
      matches.push(match[1]);
    }
  }
  if (extension === ".html") {
    for (const match of contents.matchAll(/(?:href|src)="([^"]+)"/g)) {
      matches.push(match[1]);
    }
  }
  return matches.filter(
    (target) =>
      !target.startsWith("#") &&
      !target.startsWith("http://") &&
      !target.startsWith("https://") &&
      !target.startsWith("mailto:") &&
      !target.startsWith("data:"),
  );
}

const failures = [];
for (const file of roots.flatMap(walk)) {
  const extension = extname(file);
  if (extension !== ".md" && extension !== ".html") continue;
  const contents = await readFile(file, "utf8");
  for (const rawTarget of localTargets(contents, extension)) {
    const pathname = decodeURIComponent(rawTarget.split("#", 1)[0]);
    let target = resolve(dirname(file), pathname);
    if (
      !existsSync(target) &&
      file.startsWith(resolve(root, "site")) &&
      pathname.startsWith("media/")
    ) {
      target = resolve(root, "docs", pathname);
    }
    if (!existsSync(target)) {
      failures.push(`${file.slice(root.length + 1)} -> ${rawTarget}`);
    }
  }
}

if (failures.length > 0) {
  console.error(`Broken local documentation links:\n${failures.join("\n")}`);
  process.exit(1);
}

console.log("Documentation links: local targets resolve.");
