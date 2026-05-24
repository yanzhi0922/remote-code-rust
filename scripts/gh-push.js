#!/usr/bin/env node
// Push local commit to GitHub via Git Data API
// Usage: node scripts/gh-push.js <local-commit-sha> <parent-remote-sha>

const { execFileSync, execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");

const REPO = "yanzhi0922/remote-code-rust";
const BRANCH = "main";
const localSha = process.argv[2];
const parentSha = process.argv[3];
const localParentSha = process.argv[4] || parentSha;

if (!localSha || !parentSha) {
  console.error("Usage: node gh-push.js <local-commit-sha> <parent-remote-sha> [local-parent-sha]");
  process.exit(1);
}

function ghPost(endpoint, body) {
  const tmpFile = path.join(os.tmpdir(), `gh-api-${Date.now()}.json`);
  try {
    fs.writeFileSync(tmpFile, JSON.stringify(body));
    const result = execFileSync(
      "gh",
      ["api", `repos/${REPO}/git/${endpoint}`, "--method", "POST", "--input", tmpFile],
      { encoding: "utf-8", stdio: ["pipe", "pipe", "pipe"] }
    );
    return JSON.parse(result);
  } finally {
    try { fs.unlinkSync(tmpFile); } catch {}
  }
}

function ghGet(endpoint) {
  const result = execFileSync(
    "gh",
    ["api", `repos/${REPO}/git/${endpoint}`],
    { encoding: "utf-8" }
  );
  return JSON.parse(result);
}

function ghPatch(endpoint, body) {
  const tmpFile = path.join(os.tmpdir(), `gh-api-${Date.now()}.json`);
  try {
    fs.writeFileSync(tmpFile, JSON.stringify(body));
    const result = execFileSync(
      "gh",
      ["api", `repos/${REPO}/git/${endpoint}`, "--method", "PATCH", "--input", tmpFile],
      { encoding: "utf-8", stdio: ["pipe", "pipe", "pipe"] }
    );
    return JSON.parse(result);
  } finally {
    try { fs.unlinkSync(tmpFile); } catch {}
  }
}

// 1. Get list of changed files between parent and local commit
console.log("1. Getting changed files...");
const diffRaw = execFileSync(
  "git",
  ["diff-tree", "--no-commit-id", "-r", `${localParentSha}..${localSha}`],
  { encoding: "utf-8" }
);

const changes = [];
for (const line of diffRaw.trim().split("\n")) {
  if (!line) continue;
  if (!line.includes("\t")) continue;
  const [meta, filePath] = line.split("\t");
  const parts = meta.replace(/^:/, "").split(/\s+/);
  const [oldMode, newMode, oldSha, newSha, status] = parts;
  changes.push({ filePath, oldSha, newSha, status, newMode });
}
console.log(`   Found ${changes.length} changed files`);

// 2. Get the remote parent tree
console.log("2. Getting remote parent tree...");
const parentCommit = ghGet(`commits/${parentSha}`);
const parentTreeSha = parentCommit.tree.sha;
console.log(`   Parent tree SHA: ${parentTreeSha}`);

// 3. Create blobs for all changed/added files
console.log("3. Creating blobs...");
const treeEntries = [];
for (const change of changes) {
  if (change.status === "D") {
    // For deleted files, include with sha=null to remove from base_tree
    treeEntries.push({
      path: change.filePath,
      mode: change.oldMode || "100644",
      type: "blob",
      sha: null,
    });
    console.log(`   DEL ${change.filePath}`);
    continue;
  }

  // Read file content from local git (use newSha to get content)
  // Use binary encoding to avoid corrupting non-UTF8 files (images, binaries, etc.)
  let content;
  try {
    content = execFileSync("git", ["show", change.newSha], { encoding: "buffer", maxBuffer: 50 * 1024 * 1024 });
  } catch (e) {
    console.log(`   SKIP ${change.filePath} (could not read content)`);
    continue;
  }

  const mode = change.newMode || "100644";

  // Create blob — content is already a Buffer, convert directly to base64
  const blob = ghPost("blobs", {
    content: Buffer.from(content).toString("base64"),
    encoding: "base64",
  });

  treeEntries.push({
    path: change.filePath,
    mode: mode,
    type: "blob",
    sha: blob.sha,
  });
  console.log(`   BLOB ${change.filePath} -> ${blob.sha.substring(0, 8)}`);
}

// 4. Create new tree
console.log("4. Creating tree...");
const newTree = ghPost("trees", {
  base_tree: parentTreeSha,
  tree: treeEntries,
});
console.log(`   New tree SHA: ${newTree.sha}`);

// 5. Create commit
console.log("5. Creating commit...");
const commitMessage = execFileSync("git", ["log", "--format=%B", "-n", "1", localSha], {
  encoding: "utf-8",
}).trim();

const newCommit = ghPost("commits", {
  message: commitMessage,
  tree: newTree.sha,
  parents: [parentSha],
});
console.log(`   New commit SHA: ${newCommit.sha}`);

// 6. Update ref
console.log("6. Updating ref...");
ghPatch(`refs/heads/${BRANCH}`, {
  sha: newCommit.sha,
  force: false,
});
console.log(`   Updated refs/heads/${BRANCH} -> ${newCommit.sha}`);
console.log("\n✅ Push complete!");
