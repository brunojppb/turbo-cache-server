import * as fs from "node:fs";
import * as path from "node:path";
import { getState, LOGS_DIR, DECAY_PID_KEY } from "./util.mjs";

const pid = getState(DECAY_PID_KEY);

if (typeof pid === "undefined") {
  console.error(`${DECAY_PID_KEY} state could not be found`);
  process.exit(1);
}

// @TODO: Check whether the server is actually running
// Before sending a SIGTERM.
console.log(`Turbo Cache Server will be stopped on pid: ${pid}`);
process.kill(parseInt(pid));

// Read logs and output it as-is so we can debug
// any potential errors during the Turborepo remote cache API calls.
// Logs are written on a "{crate_name}.log" file
const logFile = path.resolve(LOGS_DIR, "decay.log");
console.log(`Reading Turbo Cache Server logs from ${logFile}`);
const serverLogs = fs.readFileSync(logFile, { encoding: "utf-8" });
console.log(serverLogs);

process.exit(0);
