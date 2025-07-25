import { spawn } from 'node:child_process'
import { resolve } from 'node:path'
import { LOGS_DIR, DECAY_PID_KEY, saveState, getBinaryName } from './util.mjs'

const __dirname = new URL('.', import.meta.url).pathname

const binaryName = getBinaryName()
console.log(`Starting server with binary '${binaryName}'`)

const serverBinary = resolve(__dirname, `./${binaryName}`)

const decayProcess = spawn(serverBinary, [], {
  detached: true,
  stdio: 'ignore',
  env: {
    ...process.env,
    LOGS_DIRECTORY: LOGS_DIR,
  },
})

decayProcess.unref()

const pid = decayProcess.pid?.toString()

console.log(`
Turbo Cache Server running with pid: "${pid}"
Web server logs are being written at "${LOGS_DIR}"
`)

saveState(DECAY_PID_KEY, pid)
process.exit(0)
