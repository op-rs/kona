package paths

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path"
	"path/filepath"

	"github.com/ethereum-optimism/superchain-registry/ops/internal/config"
)

func FindRepoRoot() (string, error) {
	wd, err := os.Getwd()
	if err != nil {
		return "", fmt.Errorf("failed to get working directory: %w", err)
	}

	return findRepoRootFromDir(wd)
}

func findRepoRootFromDir(wd string) (string, error) {
	abs, err := filepath.Abs(wd)
	if err != nil {
		return "", fmt.Errorf("failed to get absolute path: %w", err)
	}

	for {
		if _, err := os.Stat(path.Join(abs, ".repo-root")); err == nil {
			return abs, nil
		}

		if abs == "/" {
			return "", errors.New("not in repo")
		}

		abs = path.Dir(abs)
	}
}

func StagingDir(wd string) string {
	return path.Join(wd, ".staging")
}

func SuperchainDir(wd string, name config.Superchain) string {
	return path.Join(wd, "superchain", "configs", string(name))
}

func ChainConfig(wd string, superchain config.Superchain, shortName string) string {
	return path.Join(SuperchainDir(wd, superchain), shortName+".toml")
}

func SuperchainConfig(wd string, superchain config.Superchain) string {
	return path.Join(SuperchainDir(wd, superchain), "superchain.toml")
}

func Superchains(wd string) ([]config.Superchain, error) {
	configsDir := path.Join(wd, "superchain", "configs")

	dir, err := os.ReadDir(configsDir)
	if err != nil {
		return nil, fmt.Errorf("failed to read dir %s: %w", configsDir, err)
	}

	var superchains []config.Superchain
	for _, entry := range dir {
		if entry.IsDir() {
			superchains = append(superchains, config.MustParseSuperchain(entry.Name()))
		}
	}
	return superchains, nil
}

func ExtraDir(wd string) string {
	return path.Join(wd, "superchain", "extra")
}

func GenesisFile(wd string, superchain config.Superchain, shortName string) string {
	return path.Join(ExtraDir(wd), "genesis", string(superchain), shortName+".json.zst")
}

func AddressesFile(wd string) string {
	return path.Join(ExtraDir(wd), "addresses", "addresses.json")
}

func ValidationsDir(wd string) string {
	return path.Join(wd, "validation", "standard")
}

func ValidationsFile(wd string, superchain string) string {
	return path.Join(ValidationsDir(wd), fmt.Sprintf("standard-config-params-%s.toml", superchain))
}

func RequireDir(p string) error {
	stat, err := os.Stat(p)
	if err != nil {
		return fmt.Errorf("failed to stat %s: %w", p, err)
	}

	if !stat.IsDir() {
		return fmt.Errorf("%s is not a directory", p)
	}

	return nil
}

func RequireRoot(wd string) error {
	p := StagingDir(wd)
	if err := RequireDir(p); err != nil {
		return fmt.Errorf("not at repo root or IO error: %w", err)
	}
	return nil
}

type CollectorMatcher func(string) bool

func CollectFiles(root string, matcher CollectorMatcher) ([]string, error) {
	var out []string
	if err := filepath.WalkDir(root, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if !d.IsDir() && matcher(path) {
			out = append(out, path)
		}
		return nil
	}); err != nil {
		return nil, fmt.Errorf("failed to walk staging directory: %w", err)
	}
	return out, nil
}

func FileExtMatcher(ext string) CollectorMatcher {
	return func(s string) bool {
		return filepath.Ext(s) == ext
	}
}
