import { spawn } from "node:child_process";
import { resolve } from "node:path";
import { URL } from "node:url";
import {
  createWriteStream,
  mkdirSync,
  existsSync,
  writeFileSync,
} from "node:fs";
import { DECAY_PID_KEY, TEMP_DIR, saveState } from "./util.mjs";

const __dirname = new URL(".", import.meta.url).pathname;
const serverBinary = resolve(__dirname, "./decay");

if (!existsSync(TEMP_DIR)) {
  mkdirSync(TEMP_DIR, { recursive: true });
}

const outFile = resolve(TEMP_DIR, "out.log");
const errFile = resolve(TEMP_DIR, "error.log");
writeFileSync(outFile, "stdOuput: ");
writeFileSync(errFile, "errorOutput: ");

const stdOutput = createWriteStream(outFile, { flags: "a" });
const errOutput = createWriteStream(errFile, { flags: "a" });

const decayProcess = spawn(serverBinary, [], {
  detached: true,
  env: {
    ...process.env,
  },
});

decayProcess.stdout.pipe(stdOutput);
decayProcess.stderr.pipe(errOutput);
console.log(`writing decay output at ${outFile}`);
console.log(`writing decay errors at ${errFile}`);
decayProcess.unref();

const pid = decayProcess.pid?.toString();

console.log("Decay server running with pid: ", pid);
saveState(DECAY_PID_KEY, pid);
process.exit(0);
