#!/usr/bin/env node
import { existsSync, mkdirSync, openSync, writeFileSync, closeSync, chmodSync } from "node:fs";
import { dirname } from "node:path";
import { homedir } from "node:os";
import { generateKeyPairSync } from "node:crypto";

const DEFAULT_KEY_PATH =
  process.env.BRAWLER_AUTHOR_LICENSE_KEY_PATH ??
  `${homedir()}/.local/share/brawler/license-keys/author-license-ed25519.pem`;

function usage() {
  console.error(`Usage:
  node scripts/licensing/generate-ed25519-key.mjs [--key-path PATH] [--force]

Generates an Ed25519 private key outside the repository by default and prints the
base64 raw public key that can be embedded in the app verifier.
`);
}

function parseArgs(argv) {
  const args = {
    keyPath: DEFAULT_KEY_PATH,
    force: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") {
      usage();
      process.exit(0);
    }
    if (arg === "--force") {
      args.force = true;
      continue;
    }
    if (arg === "--key-path") {
      args.keyPath = requireValue(argv, index, arg);
      index += 1;
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  return args;
}

function requireValue(argv, index, name) {
  const value = argv[index + 1];
  if (!value || value.startsWith("--")) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function rawPublicKeyBase64(publicKey) {
  const spkiDer = publicKey.export({ format: "der", type: "spki" });
  return Buffer.from(spkiDer.subarray(spkiDer.length - 32)).toString("base64");
}

try {
  const args = parseArgs(process.argv.slice(2));
  if (existsSync(args.keyPath) && !args.force) {
    throw new Error(`Private key already exists: ${args.keyPath}`);
  }

  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  const privatePem = privateKey.export({ format: "pem", type: "pkcs8" });
  mkdirSync(dirname(args.keyPath), { recursive: true, mode: 0o700 });

  const fd = openSync(args.keyPath, args.force ? "w" : "wx", 0o600);
  try {
    writeFileSync(fd, privatePem);
  } finally {
    closeSync(fd);
  }
  chmodSync(args.keyPath, 0o600);

  console.log(`private_key_path=${args.keyPath}`);
  console.log(`public_key_base64=${rawPublicKeyBase64(publicKey)}`);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  usage();
  process.exit(1);
}
