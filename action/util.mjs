import { resolve } from "node:path";
import fs from "node:fs";
import os from "node:os";

/**
 * Read state value from the Github injected values
 * @param {string} key
 * @returns
 */
export function getState(key) {
  const githubKey = `STATE_${key}`;
  return process.env[githubKey];
}

/**
 * Append state to the Github state file
 * which can be read on subsequent Github Action commands
 * @param {string} key
 * @param {string} value
 */
export function saveState(key, value) {
  const state = `${key}=${value}`;
  const stateFilePath = process.env.GITHUB_STATE;
  if (typeof stateFilePath !== "string") {
    throw new Error("GITHUB_STATE file not available");
  }
  fs.appendFileSync(stateFilePath, state);
}

export const DECAY_PID_KEY = "DECAY_PID_KEY";
export const TEMP_DIR = resolve(os.tmpdir(), "decay");
