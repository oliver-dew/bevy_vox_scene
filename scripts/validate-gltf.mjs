import { readFile } from "node:fs/promises";
import process from "node:process";
import validator from "gltf-validator";

const paths = process.argv.slice(2);
if (paths.length === 0) {
  console.error("usage: npm run validate-gltf -- <file.glb> [...]");
  process.exit(2);
}

let errors = 0;
for (const path of paths) {
  const bytes = await readFile(path);
  const report = await validator.validateBytes(new Uint8Array(bytes), {
    uri: path,
    format: "glb",
    externalResourceFunction: async () => {
      throw new Error("exported GLBs must not reference external resources");
    },
  });
  const issues = report.issues ?? {};
  console.log(
    `${path}: ${issues.numErrors ?? 0} errors, ${issues.numWarnings ?? 0} warnings, ${issues.numInfos ?? 0} infos`,
  );
  for (const message of issues.messages ?? []) {
    if (message.severity === 0) {
      console.error(`  ${message.code}: ${message.message}`);
    }
  }
  errors += issues.numErrors ?? 0;
}

if (errors > 0) {
  process.exit(1);
}
