import { spawn } from "node:child_process";
import { resolve } from "node:path";
import { URL } from "node:url";
import { createWriteStream, mkdirSync, existsSync } from "node:fs";
import { DECAY_PID_KEY, TEMP_DIR, saveState } from "./util.mjs";

const __dirname = new URL(".", import.meta.url).pathname;
const serverBinary = resolve(__dirname, "./decay");

if (!existsSync(TEMP_DIR)) {
  mkdirSync(TEMP_DIR, { recursive: true });
}

const stdOutput = createWriteStream(resolve(TEMP_DIR, "out.log"), {
  flags: "a",
});
const errOutput = createWriteStream(resolve(TEMP_DIR, "error.log"), {
  flags: "a",
});

const decayProcess = spawn(serverBinary, [], {
  detached: true,
  stdio: "ignore",
  env: {
    ...process.env,
  },
});

decayProcess.stdout.pipe(stdOutput);
decayProcess.stderr.pipe(errOutput);
decayProcess.unref();

const pid = decayProcess.pid?.toString();

console.log("Decay server running with pid: ", pid);
saveState(DECAY_PID_KEY, pid);
