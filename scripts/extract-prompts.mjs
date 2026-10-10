// Writes the starter prompts to tests/fixtures/prompts.json so the Rust
// fixture recorder replays exactly what the welcome screen sends.
import fs from "node:fs";
import { STARTERS } from "../src/chat/starters.ts";

const out = {};
for (const [mode, groups] of Object.entries(STARTERS)) {
  out[mode] = [...groups.flatMap((g) => g.starters.map((s) => s.prompt)), "What can you do?"];
}
fs.writeFileSync(new URL("../tests/fixtures/prompts.json", import.meta.url), JSON.stringify(out, null, 1) + "\n");
console.log(Object.fromEntries(Object.entries(out).map(([k, v]) => [k, v.length])));
