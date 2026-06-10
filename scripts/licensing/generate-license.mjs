#!/usr/bin/env node
import { mkdirSync, readFileSync, writeFileSync, chmodSync } from "node:fs";
import { dirname } from "node:path";
import { homedir } from "node:os";
import { createPrivateKey, sign } from "node:crypto";

const TOKEN_PREFIX = "BRAWLER-LIC-1";
const ALL_VERSION_RANGE = "*";
const DEFAULT_KEY_DIR = `${homedir()}/.local/share/brawler/license-keys`;
const DEFAULT_AUTHOR_KEY_PATH =
  process.env.BRAWLER_AUTHOR_LICENSE_KEY_PATH ?? `${DEFAULT_KEY_DIR}/author-license-ed25519.pem`;
const DEFAULT_FRIEND_KEY_PATH =
  process.env.BRAWLER_FRIEND_TEST_LICENSE_KEY_PATH ??
  `${DEFAULT_KEY_DIR}/friend-test-license-ed25519.pem`;

const LICENSE_TYPES = {
  author: {
    channel: "author",
    edition: "author",
    features: ["*"],
    holder: "Project Author",
    keyId: "owner_author_2026_06",
    keyPath: DEFAULT_AUTHOR_KEY_PATH,
    expiresAt: "2099-01-01T00:00:00Z",
    appVersionRange: ALL_VERSION_RANGE,
  },
  friend: {
    channel: "friend_test",
    edition: "friend",
    features: ["core"],
    holder: null,
    keyId: "owner_friend_test_2026_06",
    keyPath: DEFAULT_FRIEND_KEY_PATH,
    expiresAt: null,
    appVersionRange: ALL_VERSION_RANGE,
  },
};

function usage() {
  console.error(`Usage:
  node scripts/licensing/generate-license.mjs --type author [--out private/licenses/author.txt]
  node scripts/licensing/generate-license.mjs --type friend --holder "Friend Name" [--out private/licenses/friend.txt]

Options:
  --type author|friend
  --holder NAME
  --license-id ID
  --expires-at RFC3339
  --app-version-range RANGE   reserved for future version-limited license types
  --features CSV             default depends on type
  --key-path PATH            overrides the default external private key path
  --out PATH                 writes token to PATH with mode 0600
`);
}

function parseArgs(argv) {
  const args = {
    type: null,
    holder: null,
    licenseId: null,
    expiresAt: null,
    appVersionRange: null,
    features: null,
    keyPath: null,
    out: null,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") {
      usage();
      process.exit(0);
    }
    if (arg === "--type") {
      args.type = requireValue(argv, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--holder") {
      args.holder = requireValue(argv, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--license-id") {
      args.licenseId = requireValue(argv, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--expires-at") {
      args.expiresAt = requireValue(argv, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--app-version-range") {
      args.appVersionRange = requireValue(argv, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--features") {
      args.features = requireValue(argv, index, arg).split(",").map((item) => item.trim()).filter(Boolean);
      index += 1;
      continue;
    }
    if (arg === "--key-path") {
      args.keyPath = requireValue(argv, index, arg);
      index += 1;
      continue;
    }
    if (arg === "--out") {
      args.out = requireValue(argv, index, arg);
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

function addDaysIso(days) {
  const date = new Date();
  date.setUTCDate(date.getUTCDate() + days);
  date.setUTCHours(0, 0, 0, 0);
  return date.toISOString().replace(".000Z", "Z");
}

function slug(value) {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 40) || "holder";
}

function generatedLicenseId(type, holder) {
  const stamp = new Date().toISOString().slice(0, 10).replace(/-/g, "");
  return `lic_${type}_${stamp}_${slug(holder)}`;
}

function base64Url(buffer) {
  return Buffer.from(buffer).toString("base64url");
}

function buildClaims(args) {
  const template = LICENSE_TYPES[args.type];
  if (!template) {
    throw new Error("--type must be author or friend");
  }
  if (args.appVersionRange && matchesCurrentAllVersionChannel(args.type)) {
    throw new Error(`${args.type} licenses are not version-bounded; do not pass --app-version-range`);
  }
  const holder = args.holder ?? template.holder;
  if (!holder) {
    throw new Error("--holder is required for friend licenses");
  }

  return {
    license_id: args.licenseId ?? generatedLicenseId(args.type, holder),
    holder,
    channel: template.channel,
    edition: template.edition,
    features: args.features ?? template.features,
    issued_at: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
    expires_at: args.expiresAt ?? template.expiresAt ?? addDaysIso(180),
    app_version_range: template.appVersionRange,
    key_id: template.keyId,
  };
}

function matchesCurrentAllVersionChannel(type) {
  return type === "author" || type === "friend";
}

function signToken(claims, keyPath) {
  const privateKey = createPrivateKey(readFileSync(keyPath, "utf8"));
  const payload = base64Url(Buffer.from(JSON.stringify(claims)));
  const signedMessage = `${TOKEN_PREFIX}.${payload}`;
  const signature = base64Url(sign(null, Buffer.from(signedMessage), privateKey));
  return `${signedMessage}.${signature}`;
}

try {
  const args = parseArgs(process.argv.slice(2));
  const template = LICENSE_TYPES[args.type];
  if (!template) {
    throw new Error("--type must be author or friend");
  }
  const keyPath = args.keyPath ?? template.keyPath;
  const claims = buildClaims(args);
  const token = signToken(claims, keyPath);

  if (args.out) {
    mkdirSync(dirname(args.out), { recursive: true, mode: 0o700 });
    writeFileSync(args.out, `${token}\n`, { mode: 0o600 });
    chmodSync(args.out, 0o600);
    console.log(`license_path=${args.out}`);
  } else {
    console.log(token);
  }

  console.error(`license_id=${claims.license_id}`);
  console.error(`holder=${claims.holder}`);
  console.error(`channel=${claims.channel}`);
  console.error(`edition=${claims.edition}`);
  console.error(`features=${claims.features.join(",")}`);
  console.error(`expires_at=${claims.expires_at}`);
  console.error(`app_version_range=${claims.app_version_range}`);
  console.error(`key_id=${claims.key_id}`);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  usage();
  process.exit(1);
}
