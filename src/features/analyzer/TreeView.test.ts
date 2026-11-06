import { describe, expect, it } from "vitest";
import { findAncestors, flattenVisible, nodeKey } from "./TreeView";
import { defaultFilters } from "../../lib/analyzerFilters";
import type { ScanNode } from "../../lib/types";

function makeTree(): ScanNode {
  return {
    name: "C:",
    path: "C:\\",
    kind: "dir",
    size: 50_000,
    allocated: 50_000,
    fileCount: 15,
    dirCount: 2,
    modified: 1700000000,
    children: [
      {
        name: "Windows",
        path: "C:\\Windows",
        kind: "dir",
        size: 30_000,
        allocated: 30_000,
        fileCount: 10,
        dirCount: 1,
        modified: 1700000000,
        children: [
          {
            name: "System32",
            path: "C:\\Windows\\System32",
            kind: "dir",
            size: 20_000,
            allocated: 20_000,
            fileCount: 8,
            dirCount: 0,
            modified: 1700000000,
            children: [
              {
                name: "cmd.exe",
                path: "C:\\Windows\\System32\\cmd.exe",
                kind: "file",
                size: 5_000,
                allocated: 5_000,
                fileCount: 1,
                dirCount: 0,
                modified: 1700000000,
                children: [],
              },
            ],
          },
        ],
      },
      {
        name: "Users",
        path: "C:\\Users",
        kind: "dir",
        size: 20_000,
        allocated: 20_000,
        fileCount: 5,
        dirCount: 0,
        children: [],
      },
    ],
  };
}

describe("TreeView regression tests", () => {
  it("keeps the root row visible when the drive is collapsed", () => {
    const root = makeTree();
    const rootKey = nodeKey(root, null, 0);

    // Completely collapsed (expanded set is empty, or does not contain rootKey)
    const collapsed = new Set<string>();
    const { rows } = flattenVisible(root, collapsed, defaultFilters);

    expect(rows).toHaveLength(1);
    expect(rows[0].node.name).toBe("C:");
    expect(rows[0].key).toBe(rootKey);
    expect(rows[0].hasChildren).toBe(true);
    expect(rows[0].level).toBe(1);
  });

  it("flattens children when root is expanded", () => {
    const root = makeTree();
    const rootKey = nodeKey(root, null, 0);

    const expanded = new Set<string>([rootKey]);
    const { rows } = flattenVisible(root, expanded, defaultFilters);

    // Root + Windows + Users = 3 rows
    expect(rows).toHaveLength(3);
    expect(rows[0].node.name).toBe("C:");
    expect(rows[1].node.name).toBe("Windows");
    expect(rows[2].node.name).toBe("Users");
  });

  it("finds ancestor keys accurately for deep nodes and prunes non-ancestor branches", () => {
    const root = makeTree();
    const targetFile = root.children[0].children[0].children[0]; // cmd.exe
    expect(targetFile.name).toBe("cmd.exe");

    const ancestors = findAncestors(root, targetFile);
    expect(ancestors).toHaveLength(3);
    expect(ancestors).toContain(nodeKey(root, null, 0));
    expect(ancestors).toContain(nodeKey(root.children[0], nodeKey(root, null, 0), 0));
    expect(ancestors).toContain(
      nodeKey(root.children[0].children[0], nodeKey(root.children[0], nodeKey(root, null, 0), 0), 0),
    );
  });

  it("handles multi-root workspaces with independent volume expansion", () => {
    const driveC = makeTree();
    const driveD: ScanNode = {
      name: "D:",
      path: "D:\\",
      kind: "dir",
      size: 40_000,
      allocated: 40_000,
      fileCount: 4,
      dirCount: 1,
      modified: 1700000000,
      children: [
        {
          name: "Games",
          path: "D:\\Games",
          kind: "dir",
          size: 40_000,
          allocated: 40_000,
          fileCount: 4,
          dirCount: 0,
          children: [],
        },
      ],
    };

    const combinedRoot: ScanNode = {
      name: "All Scanned Drives",
      path: "",
      kind: "dir",
      size: driveC.size + driveD.size,
      allocated: driveC.allocated + driveD.allocated,
      fileCount: driveC.fileCount + driveD.fileCount,
      dirCount: driveC.dirCount + driveD.dirCount + 2,
      modified: null,
      children: [driveC, driveD],
    };

    const combinedKey = nodeKey(combinedRoot, null, 0);
    const cKey = nodeKey(driveC, combinedKey, 0);
    const dKey = nodeKey(driveD, combinedKey, 1);

    // Expand combined workspace root and drive D, but keep drive C collapsed
    const expanded = new Set<string>([combinedKey, dKey]);
    const { rows } = flattenVisible(combinedRoot, expanded, defaultFilters);

    // Should have: CombinedRoot, C: (collapsed, hasChildren: true), D: (expanded), D:\Games
    expect(rows).toHaveLength(4);
    expect(rows[0].node.name).toBe("All Scanned Drives");
    expect(rows[1].node.name).toBe("C:");
    expect(rows[1].hasChildren).toBe(true);
    expect(rows[2].node.name).toBe("D:");
    expect(rows[3].node.name).toBe("Games");
  });
});
