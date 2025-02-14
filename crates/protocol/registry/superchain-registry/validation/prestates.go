package validation

import (
	_ "embed"
	"fmt"

	"github.com/BurntSushi/toml"
)

type Prestates struct {
	LatestRC     string                `toml:"latest_rc"`
	LatestStable string                `toml:"latest_stable"`
	Prestates    map[string][]Prestate `toml:"prestates"`
}

func (p Prestates) StablePrestate() Prestate {
	return p.Prestates[p.LatestStable][0]
}

type Prestate struct {
	Type string `toml:"type"`
	Hash Hash   `toml:"hash"`
}

//go:embed standard/standard-prestates.toml
var standardPrestatesBytes []byte

var StandardPrestates Prestates

func init() {
	if err := toml.Unmarshal(standardPrestatesBytes, &StandardPrestates); err != nil {
		panic(fmt.Errorf("failed to unmarshal standard prestates: %w", err))
	}

	if _, ok := StandardPrestates.Prestates[StandardPrestates.LatestRC]; !ok {
		panic(fmt.Errorf("latest RC prestate not found in standard prestates"))
	}

	if _, ok := StandardPrestates.Prestates[StandardPrestates.LatestStable]; !ok {
		panic(fmt.Errorf("latest stable prestate not found in standard prestates"))
	}
}
