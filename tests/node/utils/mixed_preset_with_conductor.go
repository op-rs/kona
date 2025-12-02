package node_utils

import (
	"github.com/ethereum-optimism/optimism/op-devstack/compat"
	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-devstack/shim"
	"github.com/ethereum-optimism/optimism/op-devstack/stack"
	"github.com/ethereum-optimism/optimism/op-devstack/stack/match"
)

type MinimalWithConductors struct {
	*MixedOpKonaPreset
	ConductorSets map[stack.L2NetworkID]dsl.ConductorSet
}

// WithMixedOpKonaWithConductors returns a CommonOption for conductor tests.
// Forces Kurtosis/sysext mode since sysgo doesn't support conductors yet.
func WithMixedOpKonaWithConductors(l2NodeConfig L2NodeConfig) stack.CommonOption {
	return stack.Combine(
		stack.MakeCommon(DefaultMixedOpKonaSystem(&DefaultMixedOpKonaSystemIDs{}, l2NodeConfig)),
		// Force Kurtosis mode - sysgo doesn't support conductors (TODO: op-devstack#16418)
		presets.WithCompatibleTypes(
			compat.Persistent,
			compat.Kurtosis,
		),
	)
}

func NewMixedOpKonaWithConductors(t devtest.T) *MinimalWithConductors {
	system := shim.NewSystem(t)
	orch := presets.Orchestrator()
	orch.Hydrate(system)

	chains := system.L2Networks()
	conductorSets := make(map[stack.L2NetworkID]dsl.ConductorSet)
	for _, chain := range chains {
		chainMatcher := match.L2ChainById(chain.ID())
		l2 := system.L2Network(match.Assume(t, chainMatcher))
		conductorSets[chain.ID()] = dsl.NewConductorSet(l2.Conductors())
	}

	return &MinimalWithConductors{
		MixedOpKonaPreset: NewMixedOpKona(t),
		ConductorSets:     conductorSets,
	}
}
