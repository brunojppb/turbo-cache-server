import { spawn } from "node:child_process";
import { resolve } from "node:path";
import { createWriteStream, mkdirSync, existsSync } from "node:fs";
import { DECAY_PID_KEY, TEMP_DIR, saveState } from "./util.mjs";

const serverBinary = resolve(__dirname, "./decay");

if (!existsSync(TEMP_DIR)) {
  mkdirSync(TEMP_DIR, { recursive: true });
}

// @TODO: Check whether I actually want to give args to the binary on startup
const decayProcess = spawn(
  serverBinary,
  [
    /** input here as args once we suppor them */
  ],
  {
    detached: true,
    stdio: "ignore",
    stdout: createWriteStream(resolve(TEMP_DIR, "out.log")),
    stderr: createWriteStream(resolve(TEMP_DIR, "error.log")),
    env: {
      ...process.env,
    },
  },
);

decayProcess.unref();

const pid = decayProcess.pid?.toString();

console.log("Decay server running with pid: ", pid);
saveState(DECAY_PID_KEY, pid);