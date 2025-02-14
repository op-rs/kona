package fs

import (
	"fmt"
	"os"
)

func DirExists(p string) (bool, error) {
	stat, err := os.Stat(p)
	if err == nil {
		if !stat.IsDir() {
			return false, fmt.Errorf("%s is not a directory", p)
		}

		return true, nil
	}
	if os.IsNotExist(err) {
		return false, nil
	}
	return false, fmt.Errorf("unable to stat %s: %w", p, err)
}

func FileExists(p string) (bool, error) {
	stat, err := os.Stat(p)
	if err == nil {
		if stat.IsDir() {
			return false, fmt.Errorf("%s is a directory", p)
		}

		return true, nil
	}
	if os.IsNotExist(err) {
		return false, nil
	}
	return false, fmt.Errorf("unable to stat %s: %w", p, err)
}
