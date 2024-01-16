import { spawn } from "child_process";
import { resolve } from "path";
import { DECAY_PID_KEY, TEMP_DIR, saveState } from "./util";

const serverBinary = resolve(__dirname, "./decay");

// @TODO: Check whether I actually want to give args to the binary on startup
const subprocess = spawn(serverBinary, [/** input here as args once we suppor them */], {
  detached: true,
  stdio: "ignore",
  stdout: createWriteStream(resolve(TEMP_DIR, "out.log")),
  stderr: createWriteStream(resolve(TEMP_DIR, "error.log")), 
  env: {
    ...process.env,
  },
});

subprocess.unref();

const pid = subprocess.pid?.toString();

console.log('Decay server pid to store: ', pid);
saveState(DECAY_PID_KEY, pid);
