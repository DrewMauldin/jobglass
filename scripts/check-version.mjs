import { readFile } from "node:fs/promises";

const packageManifest = JSON.parse(await readFile("package.json", "utf8"));
const tauriManifest = JSON.parse(
  await readFile("src-tauri/tauri.conf.json", "utf8"),
);
const cargoManifest = await readFile("src-tauri/Cargo.toml", "utf8");
const cargoVersion = cargoManifest.match(/^version = "([^"]+)"$/mu)?.[1];
const versions = [packageManifest.version, tauriManifest.version, cargoVersion];

if (versions.some((version) => typeof version !== "string")) {
  throw new Error("could not read every product version");
}
if (new Set(versions).size !== 1) {
  throw new Error(`version mismatch: ${versions.join(", ")}`);
}

const expectedTag = process.argv[2];
if (expectedTag && expectedTag !== `v${versions[0]}`) {
  throw new Error(
    `tag ${expectedTag} does not match product version v${versions[0]}`,
  );
}

console.log(`version contract: v${versions[0]}`);
