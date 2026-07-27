import { describe, expect, it } from 'vitest'

import type { TreeDirectoryState, TreeNode } from '@/models/tree'
import { buildVisibleTreeItems, resolveDocumentCreateParent } from '@/models/treeView'

const timestamp = '2026-07-27T01:02:03.004Z'

function directory(name: string, path = name): TreeNode {
  return { kind: 'directory', name, path, modifiedAt: timestamp }
}

function file(name: string, path = name): TreeNode {
  return { kind: 'file', name, path, size: 10, modifiedAt: timestamp }
}

function directoryState(childPaths: string[], expanded = false): TreeDirectoryState {
  return {
    childPaths,
    expanded,
    loadStatus: 'loaded',
    error: null,
  }
}

describe('tree view projection', () => {
  it('projects expanded descendants with stable ARIA positions', () => {
    const nodes = {
      projects: directory('projects'),
      'root.md': file('root.md'),
      'projects/note.md': file('note.md', 'projects/note.md'),
    }
    const directories = {
      '': directoryState(['projects', 'root.md'], true),
      projects: directoryState(['projects/note.md'], true),
    }

    expect(buildVisibleTreeItems('', nodes, directories)).toEqual([
      {
        node: nodes.projects,
        depth: 1,
        parentPath: '',
        position: 1,
        setSize: 2,
      },
      {
        node: nodes['projects/note.md'],
        depth: 2,
        parentPath: 'projects',
        position: 1,
        setSize: 1,
      },
      {
        node: nodes['root.md'],
        depth: 1,
        parentPath: '',
        position: 2,
        setSize: 2,
      },
    ])
  })

  it('does not project collapsed descendants or follow a directory cycle', () => {
    const nodes = {
      projects: directory('projects'),
      'projects/loop': directory('loop', 'projects/loop'),
    }

    expect(
      buildVisibleTreeItems('', nodes, {
        '': directoryState(['projects'], true),
        projects: directoryState(['projects/loop'], false),
        'projects/loop': directoryState(['projects'], true),
      }).map(({ node }) => node.path),
    ).toEqual(['projects'])

    expect(
      buildVisibleTreeItems('', nodes, {
        '': directoryState(['projects'], true),
        projects: directoryState(['projects/loop'], true),
        'projects/loop': directoryState(['projects'], true),
      }).map(({ node }) => node.path),
    ).toEqual(['projects', 'projects/loop', 'projects'])
  })
})

describe('document create action context', () => {
  const nodes = {
    projects: directory('projects'),
    'projects/note.md': file('note.md', 'projects/note.md'),
    'root.md': file('root.md'),
  }

  it('uses a focused directory directly', () => {
    expect(resolveDocumentCreateParent('projects', 'root.md', nodes, '')).toBe('projects')
  })

  it('uses the focused file parent before the selected node', () => {
    expect(resolveDocumentCreateParent('projects/note.md', 'root.md', nodes, '')).toBe('projects')
  })

  it('falls back to the selected file and then root', () => {
    expect(resolveDocumentCreateParent(null, 'projects/note.md', nodes, '')).toBe('projects')
    expect(resolveDocumentCreateParent(null, null, nodes, '')).toBe('')
    expect(resolveDocumentCreateParent('missing', 'projects', nodes, '')).toBe('')
  })
})
