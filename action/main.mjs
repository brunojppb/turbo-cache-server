import { spawn } from "node:child_process";
import { resolve } from "node:path";
import {
  LOGS_DIR,
  DECAY_PID_KEY,
  saveState,
  getBinaryName,
  checkHealth,
  isProcessRunning,
} from "./util.mjs";

const __dirname = new URL(".", import.meta.url).pathname;

const binaryName = getBinaryName();
console.log(`Starting server with binary '${binaryName}'`);

const serverBinary = resolve(__dirname, `./${binaryName}`);

// Capture stdout/stderr for debugging startup failures
const decayProcess = spawn(serverBinary, [], {
  detached: true,
  stdio: ["ignore", "pipe", "pipe"],
  env: {
    ...process.env,
    LOGS_DIRECTORY: LOGS_DIR,
  },
});

const pid = decayProcess.pid;

// Collect startup output for error reporting
let startupOutput = "";
decayProcess.stdout?.on("data", (data) => {
  startupOutput += data.toString();
});
decayProcess.stderr?.on("data", (data) => {
  startupOutput += data.toString();
});

let processExited = false;
let exitCode = null;

decayProcess.on("exit", (code, signal) => {
  processExited = true;
  exitCode = code;
});

decayProcess.on("error", (err) => {
  console.error(`Failed to start decay server: ${err.message}`);
  process.exit(1);
});

// Get host and port from env (same as Rust app does)
const host = process.env.HOST || "127.0.0.1";
const port = process.env.PORT || "8000";

console.log(`Waiting for server to become healthy at ${host}:${port}...`);

// Wait for health check to pass
const isHealthy = await checkHealth(host, port, 20);

if (!isHealthy) {
  console.error("\nFailed to start Turbo Cache Server within 30 seconds.");

  if (processExited) {
    console.error(`Process exited with code: ${exitCode}`);
  } else if (isProcessRunning(pid)) {
    console.error(
      `Process is running but health check failed. Possible port conflict?`,
    );
  }

  if (startupOutput) {
    console.error("\nServer output:");
    console.error(startupOutput);
  }

  // Kill process if still running
  if (!processExited && isProcessRunning(pid)) {
    try {
      process.kill(pid, "SIGKILL");
    } catch (err) {
      // Ignore kill errors
    }
  }

  process.exit(1);
}

// Server is healthy, unref and save state
decayProcess.unref();

console.log(`
Turbo Cache Server running with pid: "${pid}"
Health check passed at ${host}:${port}
Server startup output: ${startupOutput}

Web server logs are being written at "${LOGS_DIR}"
`);

saveState(DECAY_PID_KEY, pid?.toString());
process.exit(0);
