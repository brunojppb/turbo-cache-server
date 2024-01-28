import path from "node:path";
import fs from "node:fs";
import { getState, DECAY_PID_KEY, TEMP_DIR } from "./util.mjs";

const pid = getState(DECAY_PID_KEY);

if (typeof pid === "undefined") {
  console.error(`${DECAY_PID_KEY} state could not be found`);
  process.exit(1);
}

// @TODO: Check whether the server is actually running
console.log(`Decay server will be stopped on pid: ${pid}`);
process.kill(parseInt(pid));

const stdLogs = fs.readFileSync(path.resolve(TEMP_DIR, "out.log"), {
  encoding: "utf8",
  flag: "r",
});

const errLogs = fs.readFileSync(path.resolve(TEMP_DIR, "error.log"), {
  encoding: "utf8",
  flag: "r",
});

console.log(`Server output: `, stdLogs);
console.error(`\n\nServer errors: `, errLogs);
process.exit(0);
