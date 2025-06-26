package nodedevstack

import (
	"fmt"
	"strings"

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

type L2NodeKind string

const (
	OpNode   L2NodeKind = "optimism"
	KonaNode L2NodeKind = "kona"
)

type MixedOpKonaPreset struct {
	Log          log.Logger
	T            devtest.T
	ControlPlane stack.ControlPlane

	L1Network *dsl.L1Network
	L1EL      *dsl.L1ELNode

	L2Chain   *dsl.L2Network
	L2Batcher *dsl.L2Batcher

	L2ELOpNodes []dsl.L2ELNode
	L2CLOpNodes []dsl.L2CLNode

	L2ELKonaNodes []dsl.L2ELNode
	L2CLKonaNodes []dsl.L2CLNode

	TestSequencer *dsl.TestSequencer

	Wallet *dsl.HDWallet

	FaucetL1 *dsl.Faucet
	Faucet   *dsl.Faucet
	FunderL1 *dsl.Funder
	Funder   *dsl.Funder
}

// L2CLNodes returns all the L2CL nodes in the network (op-nodes and kona-nodes).
func (m *MixedOpKonaPreset) L2CLNodes() []dsl.L2CLNode {
	return append(m.L2CLOpNodes, m.L2CLKonaNodes...)
}

// L2CLNodes returns all the L2CL nodes in the network (op-nodes and kona-nodes).
func (m *MixedOpKonaPreset) L2ELNodes() []dsl.L2ELNode {
	return append(m.L2ELOpNodes, m.L2ELKonaNodes...)
}

func L2NodeMatcher[
	I interface {
		comparable
		Key() string
	}, E stack.Identifiable[I]](value string) stack.Matcher[I, E] {
	return match.MatchElemFn[I, E](func(elem E) bool {
		return strings.Contains(elem.ID().Key(), value)
	})
}

func (m *MixedOpKonaPreset) L2Networks() []*dsl.L2Network {
	return []*dsl.L2Network{
		m.L2Chain,
	}
}

func WithMixedOpKona() stack.CommonOption {
	return stack.MakeCommon(DefaultMixedOpKonaSystem(&DefaultMixedOpKonaSystemIDs{}, 2, 2))
}

func L2CLNodes(nodes []stack.L2CLNode, orch stack.Orchestrator) []dsl.L2CLNode {
	out := make([]dsl.L2CLNode, len(nodes))
	for i, node := range nodes {
		out[i] = *dsl.NewL2CLNode(node, orch.ControlPlane())
	}
	return out
}

func L2ELNodes(nodes []stack.L2ELNode) []dsl.L2ELNode {
	out := make([]dsl.L2ELNode, len(nodes))
	for i, node := range nodes {
		out[i] = *dsl.NewL2ELNode(node)
	}
	return out
}

func NewMixedOpKona(t devtest.T) *MixedOpKonaPreset {
	system := shim.NewSystem(t)
	orch := presets.Orchestrator()
	orch.Hydrate(system)

	t.Gate().Equal(len(system.TestSequencers()), 1, "expected exactly one test sequencer")

	t.Gate().Equal(len(system.L2Networks()), 1, "expected exactly one L2 network")
	t.Gate().Equal(len(system.L1Networks()), 1, "expected exactly one L1 network")

	l1Net := system.L1Network(match.FirstL1Network)
	l2Net := system.L2Network(match.Assume(t, match.L2ChainA))

	t.Gate().GreaterOrEqual(len(l2Net.L2CLNodes()), 2, "expected at least two L2CL nodes")

	opCLNodes := L2NodeMatcher[stack.L2CLNodeID, stack.L2CLNode](string(OpNode)).Match(l2Net.L2CLNodes())
	konaCLNodes := L2NodeMatcher[stack.L2CLNodeID, stack.L2CLNode](string(KonaNode)).Match(l2Net.L2CLNodes())

	t.Gate().GreaterOrEqual(len(opCLNodes), 1, "expected at least one op-node")
	t.Gate().GreaterOrEqual(len(konaCLNodes), 1, "expected at least one kona-node")

	opELNodes := L2NodeMatcher[stack.L2ELNodeID, stack.L2ELNode](string(OpNode)).Match(l2Net.L2ELNodes())
	konaELNodes := L2NodeMatcher[stack.L2ELNodeID, stack.L2ELNode](string(KonaNode)).Match(l2Net.L2ELNodes())

	t.Gate().GreaterOrEqual(len(opELNodes), 1, "expected at least one op-node")
	t.Gate().GreaterOrEqual(len(konaELNodes), 1, "expected at least one kona-node")

	out := &MixedOpKonaPreset{
		Log:           t.Logger(),
		T:             t,
		ControlPlane:  orch.ControlPlane(),
		L1Network:     dsl.NewL1Network(system.L1Network(match.FirstL1Network)),
		L1EL:          dsl.NewL1ELNode(l1Net.L1ELNode(match.Assume(t, match.FirstL1EL))),
		L2Chain:       dsl.NewL2Network(l2Net),
		L2Batcher:     dsl.NewL2Batcher(l2Net.L2Batcher(match.Assume(t, match.FirstL2Batcher))),
		L2ELOpNodes:   L2ELNodes(opELNodes),
		L2CLOpNodes:   L2CLNodes(opCLNodes, orch),
		L2ELKonaNodes: L2ELNodes(konaELNodes),
		L2CLKonaNodes: L2CLNodes(konaCLNodes, orch),
		TestSequencer: dsl.NewTestSequencer(system.TestSequencer(match.Assume(t, match.FirstTestSequencer))),
		Wallet:        dsl.NewHDWallet(t, devkeys.TestMnemonic, 30),
		Faucet:        dsl.NewFaucet(l2Net.Faucet(match.Assume(t, match.FirstFaucet))),
	}
	return out
}

type DefaultMixedOpKonaSystemIDs struct {
	L1   stack.L1NetworkID
	L1EL stack.L1ELNodeID
	L1CL stack.L1CLNodeID

	L2 stack.L2NetworkID

	L2CLOpNodes []stack.L2CLNodeID
	L2ELOpNodes []stack.L2ELNodeID

	L2CLKonaNodes []stack.L2CLNodeID
	L2ELKonaNodes []stack.L2ELNodeID

	L2Batcher  stack.L2BatcherID
	L2Proposer stack.L2ProposerID

	TestSequencer stack.TestSequencerID
}

func NewDefaultMixedOpKonaSystemIDs(l1ID, l2ID eth.ChainID, opNodes, konaNodes int) DefaultMixedOpKonaSystemIDs {
	opCLNodes := make([]stack.L2CLNodeID, opNodes)
	opELNodes := make([]stack.L2ELNodeID, opNodes)
	konaCLNodes := make([]stack.L2CLNodeID, konaNodes)
	konaELNodes := make([]stack.L2ELNodeID, konaNodes)

	for i := 0; i < opNodes; i++ {
		opCLNodes[i] = stack.NewL2CLNodeID(fmt.Sprintf("cl-op-node-%d", i), l2ID)
		opELNodes[i] = stack.NewL2ELNodeID(fmt.Sprintf("el-op-node-%d", i), l2ID)
	}
	for i := 0; i < konaNodes; i++ {
		konaCLNodes[i] = stack.NewL2CLNodeID(fmt.Sprintf("cl-kona-node-%d", i), l2ID)
		konaELNodes[i] = stack.NewL2ELNodeID(fmt.Sprintf("el-kona-node-%d", i), l2ID)
	}

	ids := DefaultMixedOpKonaSystemIDs{
		L1:   stack.L1NetworkID(l1ID),
		L1EL: stack.NewL1ELNodeID("l1", l1ID),
		L1CL: stack.NewL1CLNodeID("l1", l1ID),
		L2:   stack.L2NetworkID(l2ID),

		L2CLOpNodes:   opCLNodes,
		L2ELOpNodes:   opELNodes,
		L2CLKonaNodes: konaCLNodes,
		L2ELKonaNodes: konaELNodes,

		L2Batcher:     stack.NewL2BatcherID("main", l2ID),
		L2Proposer:    stack.NewL2ProposerID("main", l2ID),
		TestSequencer: "test-sequencer",
	}
	return ids
}

func DefaultMixedOpKonaSystem(dest *DefaultMixedOpKonaSystemIDs, opNodes, konaNodes int) stack.Option[*sysgo.Orchestrator] {
	l1ID := eth.ChainIDFromUInt64(900)
	l2ID := eth.ChainIDFromUInt64(901)
	ids := NewDefaultMixedOpKonaSystemIDs(l1ID, l2ID, opNodes, konaNodes)

	opt := stack.Combine[*sysgo.Orchestrator]()
	opt.Add(stack.BeforeDeploy(func(o *sysgo.Orchestrator) {
		o.P().Logger().Info("Setting up")
	}))

	opt.Add(sysgo.WithMnemonicKeys(devkeys.TestMnemonic))

	opt.Add(sysgo.WithDeployer(),
		sysgo.WithDeployerOptions(
			sysgo.WithLocalContractSources(),
			sysgo.WithCommons(ids.L1.ChainID()),
			sysgo.WithPrefundedL2(ids.L1.ChainID(), ids.L2.ChainID()),
		),
	)

	opt.Add(sysgo.WithL1Nodes(ids.L1EL, ids.L1CL))

	for _, node := range ids.L2ELOpNodes {
		opt.Add(sysgo.WithL2ELNode(node, nil))
	}
	for i, node := range ids.L2CLOpNodes {
		opt.Add(sysgo.WithL2CLNode(node, true, false, ids.L1CL, ids.L1EL, ids.L2ELOpNodes[i]))
	}

	opt.Add(sysgo.WithL2ELNode(ids.L2ELKonaNodes[0], nil))
	opt.Add(sysgo.WithL2CLNode(ids.L2CLKonaNodes[0], false, false, ids.L1CL, ids.L1EL, ids.L2ELKonaNodes[0]))

	opt.Add(sysgo.WithBatcher(ids.L2Batcher, ids.L1EL, ids.L2CLOpNodes[0], ids.L2ELOpNodes[0]))
	opt.Add(sysgo.WithProposer(ids.L2Proposer, ids.L1EL, &ids.L2CLOpNodes[0], nil))

	opt.Add(sysgo.WithFaucets([]stack.L1ELNodeID{ids.L1EL}, []stack.L2ELNodeID{ids.L2ELOpNodes[0], ids.L2ELKonaNodes[0]}))

	opt.Add(sysgo.WithTestSequencer(ids.TestSequencer, ids.L1CL, ids.L2CLOpNodes[0], ids.L1EL, ids.L2ELOpNodes[0]))

	opt.Add(stack.Finally(func(orch *sysgo.Orchestrator) {
		*dest = ids
	}))

	return opt
}
