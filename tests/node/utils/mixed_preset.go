package node_utils

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/ethereum/go-ethereum/log"

	"github.com/ethereum-optimism/optimism/op-chain-ops/devkeys"
	"github.com/ethereum-optimism/optimism/op-deployer/pkg/deployer/artifacts"
	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-devstack/shim"
	"github.com/ethereum-optimism/optimism/op-devstack/stack"
	"github.com/ethereum-optimism/optimism/op-devstack/stack/match"
	"github.com/ethereum-optimism/optimism/op-devstack/sysgo"
	"github.com/ethereum-optimism/optimism/op-e2e/e2eutils/intentbuilder"
	"github.com/ethereum-optimism/optimism/op-node/rollup/sync"
	"github.com/ethereum-optimism/optimism/op-service/eth"
)

type L2NodeKind string

const (
	OpNode    L2NodeKind = "optimism"
	KonaNode  L2NodeKind = "kona"
	Sequencer L2NodeKind = "sequencer"
	Validator L2NodeKind = "validator"
)

type MixedOpKonaPreset struct {
	Log          log.Logger
	T            devtest.T
	ControlPlane stack.ControlPlane

	L1Network *dsl.L1Network
	L1EL      *dsl.L1ELNode

	L2Chain   *dsl.L2Network
	L2Batcher *dsl.L2Batcher

	L2ELKonaSequencerNodes []dsl.L2ELNode
	L2CLKonaSequencerNodes []dsl.L2CLNode

	L2ELOpSequencerNodes []dsl.L2ELNode
	L2CLOpSequencerNodes []dsl.L2CLNode

	L2ELOpValidatorNodes []dsl.L2ELNode
	L2CLOpValidatorNodes []dsl.L2CLNode

	L2ELKonaValidatorNodes []dsl.L2ELNode
	L2CLKonaValidatorNodes []dsl.L2CLNode

	Wallet *dsl.HDWallet

	FaucetL1 *dsl.Faucet
	Faucet   *dsl.Faucet
	FunderL1 *dsl.Funder
	Funder   *dsl.Funder
}

// L2ELNodes returns all the L2EL nodes in the network (op-reth, op-geth, etc.), validator and sequencer.
func (m *MixedOpKonaPreset) L2ELNodes() []dsl.L2ELNode {
	return append(m.L2ELSequencerNodes(), m.L2ELValidatorNodes()...)
}

// L2CLNodes returns all the L2CL nodes in the network (op-nodes and kona-nodes), validator and sequencer.
func (m *MixedOpKonaPreset) L2CLNodes() []dsl.L2CLNode {
	return append(m.L2CLSequencerNodes(), m.L2CLValidatorNodes()...)
}

// L2CLValidatorNodes returns all the validator L2CL nodes in the network (op-nodes and kona-nodes).
func (m *MixedOpKonaPreset) L2CLValidatorNodes() []dsl.L2CLNode {
	return append(m.L2CLOpValidatorNodes, m.L2CLKonaValidatorNodes...)
}

// L2CLSequencerNodes returns all the sequencer L2CL nodes in the network (op-nodes and kona-nodes).
func (m *MixedOpKonaPreset) L2CLSequencerNodes() []dsl.L2CLNode {
	return append(m.L2CLOpSequencerNodes, m.L2CLKonaSequencerNodes...)
}

// L2ELValidatorNodes returns all the validator L2EL nodes in the network (op-reth, op-geth, etc.).
func (m *MixedOpKonaPreset) L2ELValidatorNodes() []dsl.L2ELNode {
	return append(m.L2ELOpValidatorNodes, m.L2ELKonaValidatorNodes...)
}

// L2ELSequencerNodes returns all the sequencer L2EL nodes in the network (op-reth, op-geth, etc.).
func (m *MixedOpKonaPreset) L2ELSequencerNodes() []dsl.L2ELNode {
	return append(m.L2ELOpSequencerNodes, m.L2ELKonaSequencerNodes...)
}

func (m *MixedOpKonaPreset) L2CLKonaNodes() []dsl.L2CLNode {
	return append(m.L2CLKonaValidatorNodes, m.L2CLKonaSequencerNodes...)
}

func L2NodeMatcher[
	I interface {
		comparable
		Key() string
	}, E stack.Identifiable[I]](value ...string) stack.Matcher[I, E] {
	return match.MatchElemFn[I, E](func(elem E) bool {
		for _, v := range value {
			if !strings.Contains(elem.ID().Key(), v) {
				return false
			}
		}
		return true
	})
}

func (m *MixedOpKonaPreset) L2Networks() []*dsl.L2Network {
	return []*dsl.L2Network{
		m.L2Chain,
	}
}

func WithMixedOpKona(opSequencerNodes int, konaSequencerNodes int, opNodes int, konaNodes int) stack.CommonOption {
	return stack.MakeCommon(DefaultMixedOpKonaSystem(&DefaultMixedOpKonaSystemIDs{}, opSequencerNodes, konaSequencerNodes, opNodes, konaNodes))
}

func L2CLNodes(nodes []stack.L2CLNode, orch stack.Orchestrator) []dsl.L2CLNode {
	out := make([]dsl.L2CLNode, len(nodes))
	for i, node := range nodes {
		out[i] = *dsl.NewL2CLNode(node, orch.ControlPlane())
	}
	return out
}

func L2ELNodes(nodes []stack.L2ELNode, orch stack.Orchestrator) []dsl.L2ELNode {
	out := make([]dsl.L2ELNode, len(nodes))
	for i, node := range nodes {
		out[i] = *dsl.NewL2ELNode(node, orch.ControlPlane())
	}
	return out
}

func NewMixedOpKona(t devtest.T) *MixedOpKonaPreset {
	system := shim.NewSystem(t)
	orch := presets.Orchestrator()
	orch.Hydrate(system)

	t.Gate().Equal(len(system.L2Networks()), 1, "expected exactly one L2 network")
	t.Gate().Equal(len(system.L1Networks()), 1, "expected exactly one L1 network")

	l1Net := system.L1Network(match.FirstL1Network)
	l2Net := system.L2Network(match.Assume(t, match.L2ChainA))

	t.Gate().GreaterOrEqual(len(l2Net.L2CLNodes()), 2, "expected at least two L2CL nodes")

	opSequencerCLNodes := L2NodeMatcher[stack.L2CLNodeID, stack.L2CLNode](string(OpNode), string(Sequencer)).Match(l2Net.L2CLNodes())
	konaSequencerCLNodes := L2NodeMatcher[stack.L2CLNodeID, stack.L2CLNode](string(KonaNode), string(Sequencer)).Match(l2Net.L2CLNodes())

	opCLNodes := L2NodeMatcher[stack.L2CLNodeID, stack.L2CLNode](string(OpNode), string(Validator)).Match(l2Net.L2CLNodes())
	konaCLNodes := L2NodeMatcher[stack.L2CLNodeID, stack.L2CLNode](string(KonaNode), string(Validator)).Match(l2Net.L2CLNodes())

	t.Gate().GreaterOrEqual(len(konaCLNodes), 1, "expected at least one kona-node")

	opSequencerELNodes := L2NodeMatcher[stack.L2ELNodeID, stack.L2ELNode](string(OpNode), string(Sequencer)).Match(l2Net.L2ELNodes())
	konaSequencerELNodes := L2NodeMatcher[stack.L2ELNodeID, stack.L2ELNode](string(KonaNode), string(Sequencer)).Match(l2Net.L2ELNodes())
	opELNodes := L2NodeMatcher[stack.L2ELNodeID, stack.L2ELNode](string(OpNode), string(Validator)).Match(l2Net.L2ELNodes())
	konaELNodes := L2NodeMatcher[stack.L2ELNodeID, stack.L2ELNode](string(KonaNode), string(Validator)).Match(l2Net.L2ELNodes())

	out := &MixedOpKonaPreset{
		Log:          t.Logger(),
		T:            t,
		ControlPlane: orch.ControlPlane(),
		L1Network:    dsl.NewL1Network(system.L1Network(match.FirstL1Network)),
		L1EL:         dsl.NewL1ELNode(l1Net.L1ELNode(match.Assume(t, match.FirstL1EL))),
		L2Chain:      dsl.NewL2Network(l2Net, orch.ControlPlane()),
		L2Batcher:    dsl.NewL2Batcher(l2Net.L2Batcher(match.Assume(t, match.FirstL2Batcher))),

		L2ELOpSequencerNodes: L2ELNodes(opSequencerELNodes, orch),
		L2CLOpSequencerNodes: L2CLNodes(opSequencerCLNodes, orch),

		L2ELOpValidatorNodes: L2ELNodes(opELNodes, orch),
		L2CLOpValidatorNodes: L2CLNodes(opCLNodes, orch),

		L2ELKonaSequencerNodes: L2ELNodes(konaSequencerELNodes, orch),
		L2CLKonaSequencerNodes: L2CLNodes(konaSequencerCLNodes, orch),

		L2ELKonaValidatorNodes: L2ELNodes(konaELNodes, orch),
		L2CLKonaValidatorNodes: L2CLNodes(konaCLNodes, orch),

		Wallet: dsl.NewHDWallet(t, devkeys.TestMnemonic, 30),
		Faucet: dsl.NewFaucet(l2Net.Faucet(match.Assume(t, match.FirstFaucet))),
	}
	return out
}

type DefaultMixedOpKonaSystemIDs struct {
	L1   stack.L1NetworkID
	L1EL stack.L1ELNodeID
	L1CL stack.L1CLNodeID

	L2 stack.L2NetworkID

	L2ELOpSequencerNodes []stack.L2ELNodeID
	L2CLOpSequencerNodes []stack.L2CLNodeID

	L2ELKonaSequencerNodes []stack.L2ELNodeID
	L2CLKonaSequencerNodes []stack.L2CLNodeID

	L2CLOpNodes []stack.L2CLNodeID
	L2ELOpNodes []stack.L2ELNodeID

	L2CLKonaNodes []stack.L2CLNodeID
	L2ELKonaNodes []stack.L2ELNodeID

	L2Batcher  stack.L2BatcherID
	L2Proposer stack.L2ProposerID
}

func NewDefaultMixedOpKonaSystemIDs(l1ID, l2ID eth.ChainID, opSequencerNodes, konaSequencerNodes, opNodes, konaNodes int) DefaultMixedOpKonaSystemIDs {
	opCLNodes := make([]stack.L2CLNodeID, opNodes)
	opELNodes := make([]stack.L2ELNodeID, opNodes)
	konaCLNodes := make([]stack.L2CLNodeID, konaNodes)
	konaELNodes := make([]stack.L2ELNodeID, konaNodes)

	opSequencerCLNodes := make([]stack.L2CLNodeID, opSequencerNodes)
	opSequencerELNodes := make([]stack.L2ELNodeID, opSequencerNodes)
	konaSequencerCLNodes := make([]stack.L2CLNodeID, konaSequencerNodes)
	konaSequencerELNodes := make([]stack.L2ELNodeID, konaSequencerNodes)

	for i := range opSequencerNodes {
		opSequencerCLNodes[i] = stack.NewL2CLNodeID(fmt.Sprintf("cl-optimism-sequencer-node-%d", i), l2ID)
		opSequencerELNodes[i] = stack.NewL2ELNodeID(fmt.Sprintf("el-optimism-sequencer-node-%d", i), l2ID)
	}

	for i := range konaSequencerNodes {
		konaSequencerCLNodes[i] = stack.NewL2CLNodeID(fmt.Sprintf("cl-kona-sequencer-node-%d", i), l2ID)
		konaSequencerELNodes[i] = stack.NewL2ELNodeID(fmt.Sprintf("el-kona-sequencer-node-%d", i), l2ID)
	}

	for i := range opNodes {
		opCLNodes[i] = stack.NewL2CLNodeID(fmt.Sprintf("cl-optimism-validator-node-%d", i), l2ID)
		opELNodes[i] = stack.NewL2ELNodeID(fmt.Sprintf("el-optimism-validator-node-%d", i), l2ID)
	}

	for i := range konaNodes {
		konaCLNodes[i] = stack.NewL2CLNodeID(fmt.Sprintf("cl-kona-validator-node-%d", i), l2ID)
		konaELNodes[i] = stack.NewL2ELNodeID(fmt.Sprintf("el-kona-validator-node-%d", i), l2ID)
	}

	ids := DefaultMixedOpKonaSystemIDs{
		L1:   stack.L1NetworkID(l1ID),
		L1EL: stack.NewL1ELNodeID("l1", l1ID),
		L1CL: stack.NewL1CLNodeID("l1", l1ID),
		L2:   stack.L2NetworkID(l2ID),

		L2CLOpNodes: opCLNodes,
		L2ELOpNodes: opELNodes,

		L2CLOpSequencerNodes: opSequencerCLNodes,
		L2ELOpSequencerNodes: opSequencerELNodes,

		L2CLKonaNodes: konaCLNodes,
		L2ELKonaNodes: konaELNodes,

		L2CLKonaSequencerNodes: konaSequencerCLNodes,
		L2ELKonaSequencerNodes: konaSequencerELNodes,

		L2Batcher:  stack.NewL2BatcherID("main", l2ID),
		L2Proposer: stack.NewL2ProposerID("main", l2ID),
	}
	return ids
}

func DefaultMixedOpKonaSystem(dest *DefaultMixedOpKonaSystemIDs, opSequencerNodes, konaSequencerNodes, opNodes, konaNodes int) stack.Option[*sysgo.Orchestrator] {
	l1ID := eth.ChainIDFromUInt64(900)
	l2ID := eth.ChainIDFromUInt64(901)
	ids := NewDefaultMixedOpKonaSystemIDs(l1ID, l2ID, opSequencerNodes, konaSequencerNodes, opNodes, konaNodes)

	opt := stack.Combine[*sysgo.Orchestrator]()
	opt.Add(stack.BeforeDeploy(func(o *sysgo.Orchestrator) {
		o.P().Logger().Info("Setting up")
	}))

	opt.Add(sysgo.WithMnemonicKeys(devkeys.TestMnemonic))

	// Get artifacts path
	artifactsPath := os.Getenv("OP_DEPLOYER_ARTIFACTS")
	if artifactsPath == "" {
		panic("OP_DEPLOYER_ARTIFACTS is not set")
	}

	opt.Add(sysgo.WithDeployer(),
		sysgo.WithDeployerPipelineOption(
			sysgo.WithDeployerCacheDir(artifactsPath),
		),
		sysgo.WithDeployerOptions(
			func(_ devtest.P, _ devkeys.Keys, builder intentbuilder.Builder) {
				builder.WithL1ContractsLocator(artifacts.MustNewFileLocator(filepath.Join(artifactsPath, "src")))
				builder.WithL2ContractsLocator(artifacts.MustNewFileLocator(filepath.Join(artifactsPath, "src")))
			},
			sysgo.WithCommons(ids.L1.ChainID()),
			sysgo.WithPrefundedL2(ids.L1.ChainID(), ids.L2.ChainID()),
		),
	)

	opt.Add(sysgo.WithL1Nodes(ids.L1EL, ids.L1CL))

	// Spawn all nodes.
	for i := range ids.L2CLKonaSequencerNodes {
		opt.Add(sysgo.WithL2ELNode(ids.L2ELKonaSequencerNodes[i]))
		opt.Add(sysgo.WithKonaNode(ids.L2CLKonaSequencerNodes[i], ids.L1CL, ids.L1EL, ids.L2ELKonaSequencerNodes[i], sysgo.L2CLOptionFn(func(p devtest.P, id stack.L2CLNodeID, cfg *sysgo.L2CLConfig) {
			cfg.IsSequencer = true
			cfg.SequencerSyncMode = sync.ELSync
			cfg.VerifierSyncMode = sync.ELSync
		})))
	}

	for i := range ids.L2CLOpSequencerNodes {
		opt.Add(sysgo.WithL2ELNode(ids.L2ELOpSequencerNodes[i]))
		opt.Add(sysgo.WithOpNode(ids.L2CLOpSequencerNodes[i], ids.L1CL, ids.L1EL, ids.L2ELOpSequencerNodes[i], sysgo.L2CLOptionFn(func(p devtest.P, id stack.L2CLNodeID, cfg *sysgo.L2CLConfig) {
			cfg.IsSequencer = true
			cfg.SequencerSyncMode = sync.ELSync
			cfg.VerifierSyncMode = sync.ELSync
		})))
	}

	for i := range ids.L2CLKonaNodes {
		opt.Add(sysgo.WithL2ELNode(ids.L2ELKonaNodes[i]))
		opt.Add(sysgo.WithKonaNode(ids.L2CLKonaNodes[i], ids.L1CL, ids.L1EL, ids.L2ELKonaNodes[i], sysgo.L2CLOptionFn(func(p devtest.P, id stack.L2CLNodeID, cfg *sysgo.L2CLConfig) {
			cfg.SequencerSyncMode = sync.ELSync
			cfg.VerifierSyncMode = sync.ELSync
		})))
	}

	for i := range ids.L2ELOpNodes {
		opt.Add(sysgo.WithL2ELNode(ids.L2ELOpNodes[i]))
		opt.Add(sysgo.WithOpNode(ids.L2CLOpNodes[i], ids.L1CL, ids.L1EL, ids.L2ELOpNodes[i], sysgo.L2CLOptionFn(func(p devtest.P, id stack.L2CLNodeID, cfg *sysgo.L2CLConfig) {
			cfg.SequencerSyncMode = sync.ELSync
			cfg.VerifierSyncMode = sync.ELSync
		})))
	}

	// Connect all nodes to each other in the p2p network.
	CLNodeIDs := make([]stack.L2CLNodeID, 0, len(ids.L2CLKonaNodes)+len(ids.L2CLOpNodes)+len(ids.L2CLKonaSequencerNodes)+len(ids.L2CLOpSequencerNodes))
	ELNodeIDs := make([]stack.L2ELNodeID, 0, len(ids.L2ELKonaNodes)+len(ids.L2ELOpNodes)+len(ids.L2ELKonaSequencerNodes)+len(ids.L2ELOpSequencerNodes))
	for i := range ids.L2CLKonaSequencerNodes {
		CLNodeIDs = append(CLNodeIDs, ids.L2CLKonaSequencerNodes[i])
		ELNodeIDs = append(ELNodeIDs, ids.L2ELKonaSequencerNodes[i])
	}
	for i := range ids.L2CLOpSequencerNodes {
		CLNodeIDs = append(CLNodeIDs, ids.L2CLOpSequencerNodes[i])
		ELNodeIDs = append(ELNodeIDs, ids.L2ELOpSequencerNodes[i])
	}
	for i := range ids.L2CLKonaNodes {
		CLNodeIDs = append(CLNodeIDs, ids.L2CLKonaNodes[i])
		ELNodeIDs = append(ELNodeIDs, ids.L2ELKonaNodes[i])
	}
	for i := range ids.L2CLOpNodes {
		CLNodeIDs = append(CLNodeIDs, ids.L2CLOpNodes[i])
		ELNodeIDs = append(ELNodeIDs, ids.L2ELOpNodes[i])
	}

	for i := range CLNodeIDs {
		for j := range i {
			opt.Add(sysgo.WithL2CLP2PConnection(CLNodeIDs[i], CLNodeIDs[j]))
			opt.Add(sysgo.WithL2ELP2PConnection(ELNodeIDs[i], ELNodeIDs[j]))
		}
	}

	opt.Add(sysgo.WithBatcher(ids.L2Batcher, ids.L1EL, CLNodeIDs[0], ELNodeIDs[0]))
	opt.Add(sysgo.WithProposer(ids.L2Proposer, ids.L1EL, &CLNodeIDs[0], nil))

	opt.Add(sysgo.WithFaucets([]stack.L1ELNodeID{ids.L1EL}, []stack.L2ELNodeID{ELNodeIDs[0]}))

	opt.Add(stack.Finally(func(orch *sysgo.Orchestrator) {
		*dest = ids
	}))

	return opt
}
