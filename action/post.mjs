import * as fs from 'node:fs'
import * as path from 'node:path'
import { getState, LOGS_DIR, DECAY_PID_KEY, isProcessRunning, sleep } from './util.mjs'

let pid = getState(DECAY_PID_KEY)

if (typeof pid === 'undefined') {
  console.error(`${DECAY_PID_KEY} state could not be found. Exiting...`)
  process.exit(1)
}

pid = parseInt(pid)

console.log(`Turbo Cache Server will be stopped on pid: ${pid}`)

if (!isProcessRunning(pid)) {
  console.log(`Process ${pid} is not running. It may have already exited.`)
  // Continue to read logs anyway
} else {
  try {
    process.kill(pid, 'SIGTERM')

    const maxProcessCheckAttempts = 20
    const sleepTimeInMills = 500
    let killCounter = 0
    while (isProcessRunning(pid)) {
      if (killCounter >= maxProcessCheckAttempts) {
        console.error('Taking too long to stop. Killing it directly')
        process.kill(pid, 'SIGKILL')
        break
      }
      console.log(`Server is shutting down. Waiting ${sleepTimeInMills}ms...`)
      await sleep(sleepTimeInMills)
      killCounter = killCounter + 1
    }
  } catch (err) {
    if (err.code === 'ESRCH') {
      console.log(`Process ${pid} no longer exists. Skipping kill.`)
    } else if (err.code === 'EPERM') {
      console.error(`Permission denied to kill process ${pid}.`)
    } else {
      throw err
    }
  }
}

// Read logs and output it as-is so we can debug
// any potential errors during the Turborepo remote cache API calls.
// Logs are written on a "{crate_name}.log" file
const logFile = path.resolve(LOGS_DIR, 'decay.log')
console.log(`Reading Turbo Cache Server logs from ${logFile}`)
const serverLogs = fs.readFileSync(logFile, { encoding: 'utf-8' })
console.log(serverLogs)

process.exit(0)
