#!/usr/bin/env node
const fs = require("fs");
const p = JSON.parse(fs.readFileSync(0, "utf8"));
const cmd = (p.tool_input || {}).command || "";
fs.appendFileSync("/tmp/sb/pre.log", `${Date.now() / 1000} ${p.tool_name} ${cmd}\n`);
if (cmd.includes("claude")) { process.stderr.write("blocked by roundup spike: use agent_spawn\n"); process.exit(2); }
