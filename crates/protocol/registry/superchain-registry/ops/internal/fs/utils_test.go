package fs

import (
	"testing"

	"github.com/stretchr/testify/require"
)

func TestDirExists(t *testing.T) {
	exists, err := DirExists("testdata")
	require.NoError(t, err)
	require.True(t, exists)

	exists, err = DirExists("testdata/does-not-exist")
	require.NoError(t, err)
	require.False(t, exists)

	_, err = DirExists("testdata/file.txt")
	require.Error(t, err)
	require.ErrorContains(t, err, "is not a directory")
}

func TestFileExists(t *testing.T) {
	exists, err := FileExists("testdata/file.txt")
	require.NoError(t, err)
	require.True(t, exists)

	_, err = FileExists("testdata")
	require.Error(t, err)
	require.ErrorContains(t, err, "is a directory")

	exists, err = FileExists("testdata/does-not-exist")
	require.NoError(t, err)
	require.False(t, exists)
}
