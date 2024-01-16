import { spawn } from "child_process";
import { resolve } from "path";
import { DECAY_PID_KEY, saveState } from "./util";


const tempDir = resolve(os.tmpdir(), "decay");

const serverBinary = resolve(__dirname, "./decay");

const subprocess = spawn("node", [serverBinary], {
  detached: true,
  stdio: "ignore",
  stdout: createWriteStream(resolve(tempDir, "out.log")),
  stderr: createWriteStream(resolve(tempDir, "error.log")), 
  env: {
    ...process.env,
  },
});

subprocess.unref();

const pid = subprocess.pid?.toString();

console.log('Decay server pid to store: ', pid);
saveState(DECAY_PID_KEY, pid);