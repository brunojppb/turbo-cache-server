const path = require('node:path')
const fs = require("node:fs/promises");
const { getState, DECAY_PID_KEY } = require("./util");

const pid = getState(DECAY_PID_KEY);

if (typeof pid === 'undefined') {
  console.error(`${DECAY_PID_KEY} state could not be found`);
  process.exit(1);
}

// @TODO: Check whether the server is actually running
console.log(`Decay server pid to stop: ${pid}`)
process.kill(parseInt(pid));

function noop(error) {
  console.error(error)
  return ""
}

Promise.all([
  fs.readFile(path.resolve(TEMP_DIR, "out.log"), "utf8").catch(noop),
  fs.readFile(path.resolve(TEMP_DIR, "error.log"), "utf8").catch(noop),
]).then(([std, error]) => {
  if (error) {
    console.log(`Server output: `, std);
  }
  
  if (error) {
    console.error(`Server errors: `, err)
  }
});


