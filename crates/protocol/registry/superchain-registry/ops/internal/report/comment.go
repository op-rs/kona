package report

import (
	"bytes"
	_ "embed"
	"fmt"
	"math/big"
	"text/template"
	"time"

	"github.com/ethereum-optimism/superchain-registry/validation"
	"github.com/ethereum/go-ethereum/common"
)

// CommentMagic is used to identify
const CommentMagic = "<!--- Report Magic V1 -->"

// Create a FuncMap with your custom functions
var funcMap = template.FuncMap{
	"formatTime": func(t time.Time) string {
		return t.UTC().Format(time.RFC3339)
	},
	"deploymentTxLink": func(report L1Report) string {
		var subdomain string
		if report.DeploymentChainID != 1 {
			subdomain = "sepolia."
		}

		return fmt.Sprintf("[%s](https://%setherscan.io/tx/%s)", report.DeploymentTxHash, subdomain, report.DeploymentTxHash)
	},
	"checkmark": func(a, b string) string {
		if a == b {
			return "✅"
		}
		return "⚠️"
	},
	"checkmarkUint": func(a, b uint32) string {
		if a == b {
			return "✅"
		}
		return "⚠️"
	},
	"checkmarkUint64": func(a, b uint64) string {
		if a == b {
			return "✅"
		}
		return "⚠️"
	},
	"checkmarkHash": func(a any, b any) string {
		var aHash [32]byte

		switch v := a.(type) {
		case validation.Hash:
			aHash = v
		case common.Hash:
			aHash = v
		default:
			panic(fmt.Sprintf("unexpected type %T", a))
		}

		switch v := b.(type) {
		case validation.Hash:
			if aHash == v {
				return "✅"
			}
		case common.Hash:
			if aHash == v {
				return "✅"
			}
		default:
			break
		}

		return "⚠️"
	},
	"checkmarkBigInt": func(a, b *big.Int) string {
		if a != nil && b != nil && a.Cmp(b) == 0 {
			return "✅"
		}
		return "⚠️"
	},
	"checkmarkAddr": func(a validation.Address, b common.Address) string {
		if common.Address(a) == b {
			return "✅"
		}
		return "⚠️"
	},
}

//go:embed comment.md.tmpl
var commentTemplateData string

var tmpl = template.Must(template.New("comment").Funcs(funcMap).Parse(commentTemplateData))

// RenderComment renders the comment.md template with the given report and standard params
func RenderComment(
	report *Report,
	stdConfigs validation.ConfigParams,
	stdRoles validation.RolesConfig,
	stdPrestate validation.Prestate,
	stdVersions validation.VersionConfig,
	gitSHA string,
) (string, error) {
	var buf bytes.Buffer
	data := struct {
		Report      *Report
		StdConfig   validation.ConfigParams
		StdVersions validation.VersionConfig
		StdRoles    validation.RolesConfig
		StdPrestate validation.Prestate
		Magic       string
		GitSHA      string
	}{
		Report:      report,
		StdConfig:   stdConfigs,
		StdRoles:    stdRoles,
		StdPrestate: stdPrestate,
		StdVersions: stdVersions,
		Magic:       CommentMagic,
		GitSHA:      gitSHA,
	}
	if err := tmpl.Execute(&buf, data); err != nil {
		return "", err
	}
	return buf.String(), nil
}
