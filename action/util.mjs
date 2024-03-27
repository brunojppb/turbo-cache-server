import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'

/**
 * Read state value from the Github injected values
 * @param {string} key
 * @returns
 */
export function getState(key) {
  const githubKey = `STATE_${key}`
  return process.env[githubKey]
}

/**
 * Append state to the Github state file
 * which can be read on subsequent Github Action commands
 * @param {string} key
 * @param {string} value
 */
export function saveState(key, value) {
  const state = `${key}=${value}`
  const stateFilePath = process.env.GITHUB_STATE
  if (typeof stateFilePath !== 'string') {
    throw new Error('GITHUB_STATE file not available')
  }
  fs.appendFileSync(stateFilePath, state)
}

/**
 * Check whether the given process ID is still running
 * @param {number} pid
 */
export function isProcessRunning(pid) {
  try {
    // If sig is 0, then no signal is sent, but existence and permission
    // checks are still performed; this can be used to check for the
    // existence of a process ID or process group ID that the caller is
    // permitted to signal.
    // See: https://man7.org/linux/man-pages/man2/kill.2.html
    return process.kill(pid, 0)
  } catch (error) {
    return error.code === 'EPERM'
  }
}

/**
 * Sleep for the given time in milliseconds
 * @param {number} timeInMills
 */
export function sleep(timeInMills) {
  return new Promise(resolve => setTimeout(resolve, timeInMills))
}

export const LOGS_DIR = path.resolve(os.tmpdir(), 'decay_logs')
export const DECAY_PID_KEY = 'DECAY_SERVER_PID'