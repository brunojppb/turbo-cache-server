import path from "node:path";
import fs from "node:fs/promises";
import { getState, DECAY_PID_KEY, TEMP_DIR } from "./util.mjs";

const pid = getState(DECAY_PID_KEY);

if (typeof pid === "undefined") {
  console.error(`${DECAY_PID_KEY} state could not be found`);
  process.exit(1);
}

// @TODO: Check whether the server is actually running
console.log(`Decay server will be stopped on pid: ${pid}`);
process.kill(parseInt(pid));

function noop(error) {
  console.error(error);
  return "error reading output file";
}

Promise.all([
  fs.readFile(path.resolve(TEMP_DIR, "out.log"), "utf8").catch(noop),
  fs.readFile(path.resolve(TEMP_DIR, "error.log"), "utf8").catch(noop),
]).then(([std, error]) => {
  if (std) {
    console.log(`Server output: `, std);
  }

  if (error) {
    console.error(`Server errors: `, err);
  }
});