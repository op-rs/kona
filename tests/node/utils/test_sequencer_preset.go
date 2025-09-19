package node_utils

import (
	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-devstack/shim"
	"github.com/ethereum-optimism/optimism/op-devstack/stack"
	"github.com/ethereum-optimism/optimism/op-devstack/stack/match"
	"github.com/ethereum-optimism/optimism/op-devstack/sysgo"
	"github.com/ethereum-optimism/optimism/op-service/eth"
)

type MinimalWithTestSequencersPreset struct {
	*MixedOpKonaPreset

	TestSequencer dsl.TestSequencer
}

func WithMixedWithTestSequencer(konaNodes int) stack.CommonOption {
	return stack.MakeCommon(DefaultMixedWithTestSequencer(&DefaultMinimalWithTestSequencerIds{}, konaNodes))
}

func NewMixedOpKonaWithTestSequencer(t devtest.T) *MinimalWithTestSequencersPreset {
	system := shim.NewSystem(t)
	orch := presets.Orchestrator()
	orch.Hydrate(system)

	t.Gate().Equal(len(system.L2Networks()), 1, "expected exactly one L2 network")
	t.Gate().Equal(len(system.L1Networks()), 1, "expected exactly one L1 network")

	TestSequencer :=
		dsl.NewTestSequencer(system.TestSequencer(match.Assume(t, match.FirstTestSequencer)))

	return &MinimalWithTestSequencersPreset{
		MixedOpKonaPreset: NewMixedOpKona(t),
		TestSequencer:     *TestSequencer,
	}
}

type DefaultMinimalWithTestSequencerIds struct {
	DefaultMixedOpKonaSystemIDs DefaultMixedOpKonaSystemIDs
	TestSequencerId             stack.TestSequencerID
}

func NewDefaultMinimalWithTestSequencerIds(konaNodes int) DefaultMinimalWithTestSequencerIds {
	return DefaultMinimalWithTestSequencerIds{
		DefaultMixedOpKonaSystemIDs: NewDefaultMixedOpKonaSystemIDs(eth.ChainIDFromUInt64(DefaultL1ID), eth.ChainIDFromUInt64(DefaultL2ID), 1, 0, 1, konaNodes),
		TestSequencerId:             "test-sequencer",
	}
}

func DefaultMixedWithTestSequencer(dest *DefaultMinimalWithTestSequencerIds, konaNodes int) stack.Option[*sysgo.Orchestrator] {

	opt := DefaultMixedOpKonaSystem(&dest.DefaultMixedOpKonaSystemIDs, 1, 0, 1, konaNodes)

	ids := NewDefaultMinimalWithTestSequencerIds(konaNodes)

	L2SequencerCLNodes := ids.DefaultMixedOpKonaSystemIDs.L2CLOpSequencerNodes
	L2SequencerELNodes := ids.DefaultMixedOpKonaSystemIDs.L2ELOpSequencerNodes

	opt.Add(sysgo.WithTestSequencer(ids.TestSequencerId, ids.DefaultMixedOpKonaSystemIDs.L1CL, L2SequencerCLNodes[0], ids.DefaultMixedOpKonaSystemIDs.L1EL, L2SequencerELNodes[0]))

	opt.Add(stack.Finally(func(orch *sysgo.Orchestrator) {
		*dest = ids
	}))

	return opt
}
