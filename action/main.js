const { spawn } = require("node:child_process");
const { resolve } = require("node:path");
const { createWriteStream, mkdirSync, existsSync } = require('node:fs')
const { DECAY_PID_KEY, TEMP_DIR, saveState } = require("./util");

const serverBinary = resolve(__dirname, "./decay");

if (!existsSync(TEMP_DIR)) {
  mkdirSync(TEMP_DIR, { recursive: true });
}

// @TODO: Check whether I actually want to give args to the binary on startup
const decayProcess = spawn(serverBinary, [/** input here as args once we suppor them */], {
  detached: true,
  stdio: "ignore",
  stdout: createWriteStream(resolve(TEMP_DIR, "out.log")),
  stderr: createWriteStream(resolve(TEMP_DIR, "error.log")), 
  env: {
    ...process.env,
  },
});

decayProcess.unref();

const pid = decayProcess.pid?.toString();

console.log('Decay server running with pid: ', pid);
saveState(DECAY_PID_KEY, pid);
