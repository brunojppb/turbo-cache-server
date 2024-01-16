const fs = require("node:fs/promises");
const { getState, DECAY_PID_KEY } = require("./util");

const pid = getState(DECAY_PID_KEY);

if (typeof pid === 'undefined') {
  console.error(`${DECAY_PID_KEY} state could not be found`);
  process.exit(1);
}

// @TODO: Check whether the process is actually running
console.log(`Decay server pid to stop: ${pid}`)
process.kill(parseInt(pid));

Promise.all([
  fs.readFile(resolve(TEMP_DIR, "out.log"), "utf8").catch((e) => console.error(e)),
  fs.readFile(resolve(TEMP_DIR, "error.log"), "utf8").catch((e) => console.error(e)),
]).then(([std, error]) => {
  if (error) {
    console.log(`Server output: `, std);
  }
  
  if (error) {
    console.error(`Server errors: `, err)
  }
});


