package paths

import (
	"path"
	"strings"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestFindRepoRoot(t *testing.T) {
	p, err := findRepoRootFromDir("testdata/pathtest-ok/subdir1/subdir2")
	require.NoError(t, err)
	require.True(t, strings.HasSuffix(p, "testdata/pathtest-ok"))
	require.True(t, path.IsAbs(p))
	tmpDir := t.TempDir()
	_, err = findRepoRootFromDir(tmpDir)
	require.ErrorContains(t, err, "not in repo")
}
