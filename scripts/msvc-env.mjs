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
const [command, ...args] = process.argv.slice(2);

if (!command) {
  console.error("usage: node scripts/msvc-env.mjs <command> [args...]");
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
const quote = (value) => (/\s/.test(value) ? `"${value.replaceAll('"', '\\"')}"` : value);
const wrapInShell = isWindows && ["npm", "npx", "pnpm", "yarn"].includes(command);

const result = wrapInShell
  ? spawnSync(
      "cmd.exe",
      ["/d", "/s", "/c", [command, ...args].map(quote).join(" ")],
      { stdio: "inherit", env: environment },
    )
  : spawnSync(command, args, {
      stdio: "inherit",
      env: environment,
      shell: isWindows && /\.(cmd|bat)$/i.test(command),
    });

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}
process.exit(result.status ?? 1);
