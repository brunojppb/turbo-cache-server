import { spawn } from "node:child_process";
import { resolve } from "node:path";
import { URL } from "node:url";
import {
  createWriteStream,
  mkdirSync,
  existsSync,
  writeFileSync,
} from "node:fs";
import { DECAY_PID_KEY, TEMP_DIR, saveState } from "./util.mjs";

const __dirname = new URL(".", import.meta.url).pathname;
const serverBinary = resolve(__dirname, "./decay");

if (!existsSync(TEMP_DIR)) {
  mkdirSync(TEMP_DIR, { recursive: true });
}

const outFile = resolve(TEMP_DIR, "out.log");

const decayProcess = spawn(serverBinary, [">", outFile, "2>&1"], {
  detached: true,
  env: {
    ...process.env,
  },
});

decayProcess.unref();

const pid = decayProcess.pid?.toString();

console.log("Decay server running with pid: ", pid);
saveState(DECAY_PID_KEY, pid);
process.exit(0);
