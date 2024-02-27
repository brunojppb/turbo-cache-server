const { spawn } = require("node:child_process");
const { resolve } = require("node:path");
const { createWriteStream, mkdirSync, existsSync } = require("node:fs");
const { DECAY_PID_KEY, LOGS_DIR, saveState } = require("./util");

const serverBinary = resolve(__dirname, "./decay");

if (!existsSync(LOGS_DIR)) {
  mkdirSync(LOGS_DIR, { recursive: true });
}

const decayProcess = spawn(serverBinary, [], {
  detached: true,
  env: {
    ...process.env,
    LOGS_DIRECTORY: LOGS_DIR,
  },
});

decayProcess.unref();

const pid = decayProcess.pid?.toString();

console.log(`
Turbo Cache Server running with pid: "${pid}"
Web server logs are being written at "${LOGS_DIR}"
`);

saveState(DECAY_PID_KEY, pid);
