import * as fs from "node:fs";
import * as path from "node:path";
import { exec } from "node:child_process";
import { getState, LOGS_DIR, DECAY_PID_KEY } from "./util.mjs";

const pid = getState(DECAY_PID_KEY);

if (typeof pid === "undefined") {
  console.error(`${DECAY_PID_KEY} state could not be found`);
  process.exit(1);
}

// @TODO: Check whether the server is actually running
// Before sending a SIGTERM.
console.log(`Turbo Cache Server will be stopped on pid: ${pid}`);
try {
  process.kill(parseInt(pid));
} catch (err) {
  console.error(`Error stopping Turbo Cache Server: ${err.message}`);

  console.log('Trying to find process named "decay"');
  isRunning("decay.exe", "decay", "decay")
    .then((status) => {
      console.log('Process "decay" is running: ', status);
      process.exit(1);
    })
    .catch((err) => {
      // Read logs and output it as-is so we can debug
      // any potential errors during the Turborepo remote cache API calls.
      // Logs are written on a "{crate_name}.log" file
      const logFile = path.resolve(LOGS_DIR, "decay.log");
      console.log(`Reading Turbo Cache Server logs from ${logFile}`);
      const serverLogs = fs.readFileSync(logFile, { encoding: "utf-8" });
      console.log(serverLogs);

      process.exit(0);
    });
}

/**
 *
 *
 * @param {String} win
 * @param {String} mac
 * @param {String} linux
 * @return {*}
 */
function isRunning(win, mac, linux) {
  return new Promise(function (resolve, reject) {
    const plat = process.platform;
    const cmd =
      plat == "win32"
        ? "tasklist"
        : plat == "darwin"
        ? "ps -ax | grep " + mac
        : plat == "linux"
        ? "ps -A"
        : "";
    const proc =
      plat == "win32"
        ? win
        : plat == "darwin"
        ? mac
        : plat == "linux"
        ? linux
        : "";
    if (cmd === "" || proc === "") {
      resolve(false);
    }
    exec(cmd, function (err, stdout, stderr) {
      resolve(stdout.toLowerCase().indexOf(proc.toLowerCase()) > -1);
    });
  });
}
