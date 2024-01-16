const { getState, DECAY_PID_KEY } = require("./util");

const pid = getState(DECAY_PID_KEY);

if (typeof pid === 'undefined') {
  console.error(`${DECAY_PID_KEY} state could not be found`);
  process.exit(1);
}

// @TODO: Check whether the process is actually running
console.log(`Decay server pid to stop: ${pid}`)
process.kill(parseInt(pid));

// @TODO: Output the logs from our decay server
// const [std, error] = await Promise.all([
//   readFile(resolve(tempDir, "out.log"), "utf8").catch((e) => console.error(e)),
//   readFile(resolve(tempDir, "error.log"), "utf8").catch((e) => console.error(e)),
// ]);