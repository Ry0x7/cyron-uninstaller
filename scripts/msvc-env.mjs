#!/usr/bin/env node
// Runs a command with the Visual Studio C++ toolchain on PATH/LIB/INCLUDE.
// Rust on Windows needs an MSVC linker and the Windows SDK import libraries;
// a plain shell often has neither, and Git's `link.exe` shadows the real one.
// The discovery is portable (vswhere); when no toolchain is found the command
// runs with the inherited environment, which is correct on prepared CI images.
import { spawnSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import path from "node:path";

const isWindows = process.platform === "win32";
// Only the toolchain front-ends this repository actually runs are accepted.
// The executable name always comes from this table, never from argv.
const ALLOWED_COMMANDS = new Map([
  ["cargo", "cargo"],
  ["rustc", "rustc"],
  ["rustup", "rustup"],
  ["npm", "npm"],
  ["npx", "npx"],
  ["pnpm", "pnpm"],
  ["yarn", "yarn"],
]);
const [requested, ...args] = process.argv.slice(2);
const command = ALLOWED_COMMANDS.get(requested);

if (!command) {
  console.error(
    `usage: node scripts/msvc-env.mjs <${[...ALLOWED_COMMANDS.keys()].join("|")}> [args...]`,
  );
  process.exit(2);
}

function latestVersion(directory) {
  if (!existsSync(directory)) return undefined;
  return readdirSync(directory)
    .filter((name) => /^\d+(\.\d+)*$/.test(name))
    .sort((a, b) => {
      const left = a.split(".").map(Number);
      const right = b.split(".").map(Number);
      for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
        const delta = (left[index] ?? 0) - (right[index] ?? 0);
        if (delta !== 0) return delta;
      }
      return 0;
    })
    .at(-1);
}

function withToolchain(base) {
  if (!isWindows) return base;
  const programFilesX86 = base["ProgramFiles(x86)"] ?? "C:\\Program Files (x86)";
  const vswhere = path.join(
    programFilesX86,
    "Microsoft Visual Studio",
    "Installer",
    "vswhere.exe",
  );
  if (!existsSync(vswhere)) return base;

  const query = spawnSync(
    vswhere,
    [
      "-latest",
      "-products",
      "*",
      "-requires",
      "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
      "-property",
      "installationPath",
    ],
    { encoding: "utf8" },
  );
  const installation = query.stdout?.trim();
  if (!installation) return base;

  const msvcRoot = path.join(installation, "VC", "Tools", "MSVC");
  const msvcVersion = latestVersion(msvcRoot);
  const sdkRoot = path.join(programFilesX86, "Windows Kits", "10");
  const sdkVersion = latestVersion(path.join(sdkRoot, "Lib"));

  const pathParts = [];
  const includeParts = [];
  const libParts = [];

  if (msvcVersion) {
    pathParts.push(path.join(msvcRoot, msvcVersion, "bin", "Hostx64", "x64"));
    includeParts.push(path.join(msvcRoot, msvcVersion, "include"));
    libParts.push(path.join(msvcRoot, msvcVersion, "lib", "x64"));
  }
  if (sdkVersion) {
    pathParts.push(path.join(sdkRoot, "bin", sdkVersion, "x64"));
    for (const area of ["um", "ucrt", "shared", "winrt"]) {
      const include = path.join(sdkRoot, "Include", sdkVersion, area);
      if (existsSync(include)) includeParts.push(include);
    }
    for (const area of ["um", "ucrt"]) {
      const lib = path.join(sdkRoot, "Lib", sdkVersion, area, "x64");
      if (existsSync(lib)) libParts.push(lib);
    }
  }

  // Find Cargo home directory if present to guarantee cargo is resolvable
  const cargoHome = base.CARGO_HOME || (base.USERPROFILE ? path.join(base.USERPROFILE, ".cargo") : undefined);
  if (cargoHome) {
    const cargoBin = path.join(cargoHome, "bin");
    if (existsSync(cargoBin)) {
      pathParts.push(cargoBin);
    }
  }

  const environment = { ...base };
  const existingPathKey = Object.keys(environment).find((k) => k.toUpperCase() === "PATH");
  const existingPath = existingPathKey ? environment[existingPathKey] : "";
  for (const key of Object.keys(environment)) {
    if (key.toUpperCase() === "PATH") {
      delete environment[key];
    }
  }
  const combinedPath = [...pathParts, existingPath].filter(Boolean).join(path.delimiter);
  environment.PATH = combinedPath;
  environment.Path = combinedPath;

  const existingIncKey = Object.keys(environment).find((k) => k.toUpperCase() === "INCLUDE");
  const existingInc = existingIncKey ? environment[existingIncKey] : "";
  if (includeParts.length) {
    const combinedInc = [...includeParts, existingInc].filter(Boolean).join(";");
    for (const key of Object.keys(environment)) {
      if (key.toUpperCase() === "INCLUDE") delete environment[key];
    }
    environment.INCLUDE = combinedInc;
  }

  const existingLibKey = Object.keys(environment).find((k) => k.toUpperCase() === "LIB");
  const existingLib = existingLibKey ? environment[existingLibKey] : "";
  if (libParts.length) {
    const combinedLib = [...libParts, existingLib].filter(Boolean).join(";");
    for (const key of Object.keys(environment)) {
      if (key.toUpperCase() === "LIB") delete environment[key];
    }
    environment.LIB = combinedLib;
  }
  return environment;
}

const environment = withToolchain(process.env);
// npm-style launchers are .cmd shims on Windows. Run their JavaScript entry
// point with this Node binary instead, so no argument is reparsed by cmd.exe.
const npmEntry = {
  npm: "npm-cli.js",
  npx: "npx-cli.js",
}[command];
const npmScript = isWindows && npmEntry
  ? path.join(path.dirname(process.execPath), "node_modules", "npm", "bin", npmEntry)
  : undefined;

// Anything that still has to go through cmd.exe (pnpm/yarn shims) must not
// carry characters cmd.exe treats as syntax.
const CMD_META = /[&|<>^%!"\r\n]/;
function assertCmdSafe(values) {
  const unsafe = values.find((value) => CMD_META.test(value));
  if (unsafe !== undefined) {
    console.error(`Refusing to pass ${JSON.stringify(unsafe)} through cmd.exe.`);
    process.exit(2);
  }
}
const quote = (value) => (/\s/.test(value) ? `"${value}"` : value);
const needsCmd =
  isWindows &&
  !(npmScript && existsSync(npmScript)) &&
  ["npm", "npx", "pnpm", "yarn"].includes(command);

let result;
if (npmScript && existsSync(npmScript)) {
  result = spawnSync(process.execPath, [npmScript, ...args], { stdio: "inherit", env: environment });
} else if (needsCmd) {
  assertCmdSafe([command, ...args]);
  result = spawnSync("cmd.exe", ["/d", "/s", "/c", [command, ...args].map(quote).join(" ")], {
    stdio: "inherit",
    env: environment,
  });
} else {
  result = spawnSync(command, args, { stdio: "inherit", env: environment });
}

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}
process.exit(result.status ?? 1);
