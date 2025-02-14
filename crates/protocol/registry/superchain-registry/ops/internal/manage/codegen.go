package manage

import (
	"bytes"
	_ "embed"
	"encoding/json"
	"fmt"
	"path"
	"strconv"
	"text/template"
	"time"

	"github.com/BurntSushi/toml"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/config"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/fs"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/output"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/paths"
)

//go:embed chains.md.tmpl
var chainsReadmeTemplateData string

var funcMap = template.FuncMap{
	"checkmark": func(in bool) string {
		if in {
			return "✅"
		}

		return "❌"
	},
	"optedInSuperchain": func(in *uint64) string {
		if in != nil && *in < uint64(time.Now().Unix()) {
			return "✅"
		}

		return "❌"
	},
}

var tmpl = template.Must(template.New("chains-readme").Funcs(funcMap).Parse(chainsReadmeTemplateData))

func GenAllCode(rootP string) error {
	if err := GenAddressesFile(rootP); err != nil {
		return fmt.Errorf("error generating addresses file: %w", err)
	}
	output.WriteOK("generated addresses file")
	if err := GenChainListFile(rootP, path.Join(rootP, "chainList.json")); err != nil {
		return fmt.Errorf("error generating JSON chain list: %w", err)
	}
	output.WriteOK("generated JSON chain list")
	if err := GenChainListFile(rootP, path.Join(rootP, "chainList.toml")); err != nil {
		return fmt.Errorf("error generating TOML chain list: %w", err)
	}
	output.WriteOK("generated TOML chain list")
	if err := GenChainsReadme(rootP, path.Join(rootP, "CHAINS.md")); err != nil {
		return fmt.Errorf("error generating chains readme: %w", err)
	}
	output.WriteOK("generated chains readme")
	return nil
}

func GenAddressesFile(rootP string) error {
	superchains, err := paths.Superchains(rootP)
	if err != nil {
		return fmt.Errorf("error getting superchains: %w", err)
	}

	addrs := make(config.AddressesJSON)
	for _, superchain := range superchains {
		cfgs, err := CollectChainConfigs(paths.SuperchainDir(rootP, superchain))
		if err != nil {
			return fmt.Errorf("error collecting chain configs: %w", err)
		}

		for _, cfg := range cfgs {
			addrs[strconv.FormatUint(cfg.Config.ChainID, 10)] = &config.AddressesWithRoles{
				Addresses: cfg.Config.Addresses,
				Roles:     cfg.Config.Roles,
			}
		}
	}

	data, err := json.MarshalIndent(addrs, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to marshal addresses: %w", err)
	}

	if err := fs.AtomicWrite(paths.AddressesFile(rootP), 0o755, data); err != nil {
		return fmt.Errorf("failed to write addresses file: %w", err)
	}

	return nil
}

func GenChainListFile(rootP string, outP string) error {
	superchains, err := paths.Superchains(rootP)
	if err != nil {
		return fmt.Errorf("error getting superchains: %w", err)
	}

	ext := path.Ext(outP)
	var marshaler func([]config.ChainListEntry) ([]byte, error)
	switch ext {
	case ".json":
		marshaler = func(entries []config.ChainListEntry) ([]byte, error) {
			return json.MarshalIndent(entries, "", "  ")
		}
	case ".toml":
		marshaler = func(entries []config.ChainListEntry) ([]byte, error) {
			return toml.Marshal(config.ChainListTOML{
				Chains: entries,
			})
		}
	default:
		return fmt.Errorf("unsupported file extension: %s", ext)
	}

	var chains []config.ChainListEntry
	for _, superchain := range superchains {
		cfgs, err := CollectChainConfigs(paths.SuperchainDir(rootP, superchain))
		if err != nil {
			return fmt.Errorf("error collecting chain configs: %w", err)
		}

		for _, cfg := range cfgs {
			chains = append(chains, cfg.Config.ChainListEntry(superchain, cfg.ShortName))
		}
	}

	data, err := marshaler(chains)
	if err != nil {
		return fmt.Errorf("failed to marshal chain list: %w", err)
	}

	if err := fs.AtomicWrite(outP, 0o755, data); err != nil {
		return fmt.Errorf("failed to write chain list file: %w", err)
	}

	return nil
}

type ChainsReadmeData struct {
	Superchains []config.Superchain
	ChainData   [][]*config.Chain
}

func GenChainsReadme(rootP string, outP string) error {
	superchains, err := paths.Superchains(rootP)
	if err != nil {
		return fmt.Errorf("error getting superchains: %w", err)
	}

	var data ChainsReadmeData

	for _, superchain := range superchains {
		cfgs, err := CollectChainConfigs(paths.SuperchainDir(rootP, superchain))
		if err != nil {
			return fmt.Errorf("error collecting chain configs: %w", err)
		}

		chainData := make([]*config.Chain, len(cfgs))
		for i, cfg := range cfgs {
			chainData[i] = cfg.Config
		}

		data.Superchains = append(data.Superchains, superchain)
		data.ChainData = append(data.ChainData, chainData)
	}

	var buf bytes.Buffer
	if err := tmpl.Execute(&buf, data); err != nil {
		return fmt.Errorf("failed to execute template: %w", err)
	}

	if err := fs.AtomicWrite(outP, 0o755, buf.Bytes()); err != nil {
		return fmt.Errorf("failed to write chains readme: %w", err)
	}

	return nil
}
