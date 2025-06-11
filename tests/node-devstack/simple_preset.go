package nodedevstack

import (
	"github.com/ethereum/go-ethereum/log"

	"github.com/ethereum-optimism/optimism/op-chain-ops/devkeys"
	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-devstack/shim"
	"github.com/ethereum-optimism/optimism/op-devstack/stack"
	"github.com/ethereum-optimism/optimism/op-devstack/stack/match"
	"github.com/ethereum-optimism/optimism/op-devstack/sysgo"
	"github.com/ethereum-optimism/optimism/op-service/eth"
)

type SimpleKona struct {
	Log          log.Logger
	T            devtest.T
	ControlPlane stack.ControlPlane

	L1Network *dsl.L1Network
	L1EL      *dsl.L1ELNode

	L2Chain   *dsl.L2Network
	L2Batcher *dsl.L2Batcher

	L2ELOpNode *dsl.L2ELNode
	L2CLOpNode *dsl.L2CLNode

	L2ELKona *dsl.L2ELNode
	L2CLKona *dsl.L2CLNode

	TestSequencer *dsl.TestSequencer

	Wallet *dsl.HDWallet

	FaucetL1 *dsl.Faucet
	Faucet   *dsl.Faucet
	FunderL1 *dsl.Funder
	Funder   *dsl.Funder
}

func (m *SimpleKona) L2Networks() []*dsl.L2Network {
	return []*dsl.L2Network{
		m.L2Chain,
	}
}

func WithSimpleKona() stack.CommonOption {
	return stack.MakeCommon(DefaultSimpleSystem(&DefaultSimpleSystemIDs{}))
}

func NewSimpleKona(t devtest.T) *SimpleKona {
	system := shim.NewSystem(t)
	orch := presets.Orchestrator()
	orch.Hydrate(system)

	t.Gate().Equal(len(system.TestSequencers()), 1, "expected exactly one test sequencer")
	t.Gate().Equal(len(system.L2Network(match.Assume(t, match.L2ChainA)).L2CLNodes()), 2, "expected exactly two L2CL nodes")

	l1Net := system.L1Network(match.FirstL1Network)
	l2 := system.L2Network(match.Assume(t, match.L2ChainA))
	out := &SimpleKona{
		Log:           t.Logger(),
		T:             t,
		ControlPlane:  orch.ControlPlane(),
		L1Network:     dsl.NewL1Network(system.L1Network(match.FirstL1Network)),
		L1EL:          dsl.NewL1ELNode(l1Net.L1ELNode(match.Assume(t, match.FirstL1EL))),
		L2Chain:       dsl.NewL2Network(l2),
		L2Batcher:     dsl.NewL2Batcher(l2.L2Batcher(match.Assume(t, match.FirstL2Batcher))),
		L2ELOpNode:    dsl.NewL2ELNode(l2.L2ELNode(match.Assume(t, match.FirstL2EL))),
		L2CLOpNode:    dsl.NewL2CLNode(l2.L2CLNode(match.Assume(t, match.FirstL2CL)), orch.ControlPlane()),
		L2ELKona:      dsl.NewL2ELNode(l2.L2ELNode(match.Assume(t, match.SecondL2EL))),
		L2CLKona:      dsl.NewL2CLNode(l2.L2CLNode(match.Assume(t, match.SecondL2CL)), orch.ControlPlane()),
		TestSequencer: dsl.NewTestSequencer(system.TestSequencer(match.Assume(t, match.FirstTestSequencer))),
		Wallet:        dsl.NewHDWallet(t, devkeys.TestMnemonic, 30),
		Faucet:        dsl.NewFaucet(l2.Faucet(match.Assume(t, match.FirstFaucet))),
	}
	out.FaucetL1 = dsl.NewFaucet(out.L1Network.Escape().Faucet(match.Assume(t, match.FirstFaucet)))
	out.FunderL1 = dsl.NewFunder(out.Wallet, out.FaucetL1, out.L1EL)
	out.Funder = dsl.NewFunder(out.Wallet, out.Faucet, out.L2ELOpNode)
	return out
}

type DefaultSimpleSystemIDs struct {
	L1   stack.L1NetworkID
	L1EL stack.L1ELNodeID
	L1CL stack.L1CLNodeID

	L2 stack.L2NetworkID

	L2CLOpNode stack.L2CLNodeID
	L2ELOpNode stack.L2ELNodeID

	L2CLKona stack.L2CLNodeID
	L2ELKona stack.L2ELNodeID

	L2Batcher  stack.L2BatcherID
	L2Proposer stack.L2ProposerID

	TestSequencer stack.TestSequencerID
}

func NewDefaultSimpleSystemIDs(l1ID, l2ID eth.ChainID) DefaultSimpleSystemIDs {
	ids := DefaultSimpleSystemIDs{
		L1:   stack.L1NetworkID(l1ID),
		L1EL: stack.NewL1ELNodeID("l1", l1ID),
		L1CL: stack.NewL1CLNodeID("l1", l1ID),
		L2:   stack.L2NetworkID(l2ID),

		L2CLOpNode: stack.NewL2CLNodeID("sequencer", l2ID),
		L2ELOpNode: stack.NewL2ELNodeID("sequencer", l2ID),

		L2CLKona: stack.NewL2CLNodeID("kona-node", l2ID),
		L2ELKona: stack.NewL2ELNodeID("kona-node", l2ID),

		L2Batcher:     stack.NewL2BatcherID("main", l2ID),
		L2Proposer:    stack.NewL2ProposerID("main", l2ID),
		TestSequencer: "test-sequencer",
	}
	return ids
}

func DefaultSimpleSystem(dest *DefaultSimpleSystemIDs) stack.Option[*sysgo.Orchestrator] {
	l1ID := eth.ChainIDFromUInt64(900)
	l2ID := eth.ChainIDFromUInt64(901)
	ids := NewDefaultSimpleSystemIDs(l1ID, l2ID)

	opt := stack.Combine[*sysgo.Orchestrator]()
	opt.Add(stack.BeforeDeploy(func(o *sysgo.Orchestrator) {
		o.P().Logger().Info("Setting up")
	}))

	opt.Add(sysgo.WithMnemonicKeys(devkeys.TestMnemonic))

	opt.Add(sysgo.WithDeployer(),
		sysgo.WithDeployerOptions(
			sysgo.WithLocalContractSources(),
			sysgo.WithCommons(ids.L1.ChainID()),
			sysgo.WithPrefundedL2(ids.L2.ChainID()),
		),
	)

	opt.Add(sysgo.WithL1Nodes(ids.L1EL, ids.L1CL))

	opt.Add(sysgo.WithL2ELNode(ids.L2ELOpNode, nil))
	opt.Add(sysgo.WithL2CLNode(ids.L2CLOpNode, true, false, ids.L1CL, ids.L1EL, ids.L2ELOpNode))

	opt.Add(sysgo.WithL2ELNode(ids.L2ELKona, nil))
	opt.Add(sysgo.WithL2CLNode(ids.L2CLKona, false, false, ids.L1CL, ids.L1EL, ids.L2ELKona))

	opt.Add(sysgo.WithBatcher(ids.L2Batcher, ids.L1EL, ids.L2CLOpNode, ids.L2ELOpNode))
	opt.Add(sysgo.WithProposer(ids.L2Proposer, ids.L1EL, &ids.L2CLOpNode, nil))

	opt.Add(sysgo.WithFaucets([]stack.L1ELNodeID{ids.L1EL}, []stack.L2ELNodeID{ids.L2ELOpNode, ids.L2ELKona}))

	opt.Add(sysgo.WithTestSequencer(ids.TestSequencer, ids.L1CL, ids.L2CLOpNode, ids.L1EL, ids.L2ELOpNode))

	opt.Add(stack.Finally(func(orch *sysgo.Orchestrator) {
		*dest = ids
	}))

	return opt
}
