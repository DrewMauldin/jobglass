import { readdir, readFile } from "node:fs/promises";
import { gzipSync } from "node:zlib";

const budgetBytes = 200 * 1024;
const entries = await readdir(new URL("../dist/assets/", import.meta.url), {
  withFileTypes: true,
});
const scripts = entries.filter(
  (entry) => entry.isFile() && entry.name.endsWith(".js"),
);
const sizes = await Promise.all(
  scripts.map(async (entry) => {
    const contents = await readFile(
      new URL(`../dist/assets/${entry.name}`, import.meta.url),
    );
    return gzipSync(contents).byteLength;
  }),
);
const totalBytes = sizes.reduce((total, size) => total + size, 0);

if (totalBytes > budgetBytes) {
  throw new Error(
    `initial JavaScript is ${String(totalBytes)} bytes gzip; budget is ${String(budgetBytes)} bytes`,
  );
}

console.log(
  `initial JavaScript: ${String(totalBytes)} bytes gzip (${String(budgetBytes)}-byte budget)`,
);
