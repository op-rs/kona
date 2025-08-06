package reorgs

import (
	"math/rand"
	"testing"
	"time"

	"github.com/ethereum-optimism/optimism/devnet-sdk/contracts/constants"
	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-service/eth"
	"github.com/ethereum-optimism/optimism/op-service/testutils"
	"github.com/ethereum-optimism/optimism/op-service/txintent"
	"github.com/ethereum-optimism/optimism/op-service/txintent/bindings"
)

func TestInteropCycleDependency(gt *testing.T) {
	t := devtest.SerialT(gt)
	sys := presets.NewSimpleInterop(t)
	sys.L2ChainB.Escape().

	ctx := t.Ctx()

	rng := rand.New(rand.NewSource(time.Now().UnixNano()))

	alice := sys.FunderA.NewFundedEOA(eth.OneTenthEther)
	bob := sys.FunderB.NewFundedEOA(eth.OneTenthEther)

	eventLoggerB := bob.DeployEventLogger()
	eventLoggerA := alice.DeployEventLogger()

	sys.L2ChainB.CatchUpTo(sys.L2ChainA)

	// === Step 1: Alice sends initA → Chain B
	payloadA := testutils.RandomData(rng, 8)
	sendTriggerA := &txintent.SendTrigger{
		Emitter:         constants.L2ToL2CrossDomainMessenger,
		DestChainID:     bob.ChainID(),
		Target:          eventLoggerB,
		RelayedCalldata: encodeEmitLogCall(payloadA),
	}
	initA := txintent.NewIntent[*txintent.SendTrigger, *txintent.InteropOutput](alice.Plan())
	initA.Content.Set(sendTriggerA)

	// === Step 2: Bob executes Alice’s message (execB)
	execB := txintent.NewIntent[*txintent.RelayTrigger, *txintent.InteropOutput](bob.Plan())
	execB.Content.DependOn(&initA.Result)
	execB.Content.Fn(txintent.RelayIndexed(constants.L2ToL2CrossDomainMessenger, &initA.Result, &initA.PlannedTx.Included, 0))

	// === Step 3: Bob sends initB → Chain A (depends on initA, to form cycle)
	payloadB := testutils.RandomData(rng, 8)
	sendTriggerB := &txintent.SendTrigger{
		Emitter:         constants.L2ToL2CrossDomainMessenger,
		DestChainID:     alice.ChainID(),
		Target:          eventLoggerA,
		RelayedCalldata: encodeEmitLogCall(payloadB),
	}
	initB := txintent.NewIntent[*txintent.SendTrigger, *txintent.InteropOutput](bob.Plan())
	initB.Content.DependOn(&initA.Result)
	initB.Content.Set(sendTriggerB)

	// === Step 4: Alice executes Bob’s message (execA)
	execA := txintent.NewIntent[*txintent.RelayTrigger, *txintent.InteropOutput](alice.Plan())
	execA.Content.DependOn(&initB.Result)
	execA.Content.Fn(txintent.RelayIndexed(constants.L2ToL2CrossDomainMessenger, &initB.Result, &initB.PlannedTx.Included, 0))

	// === Submit all transactions in parallel without blocking
	go func() { _, _ = initA.PlannedTx.Included.Eval(ctx) }()
	go func() { _, _ = execB.PlannedTx.Included.Eval(ctx) }()
	go func() { _, _ = initB.PlannedTx.Included.Eval(ctx) }()
	go func() { _, _ = execA.PlannedTx.Included.Eval(ctx) }()

	time.Sleep(20 * time.Second)

	// === Wait for all transactions to be included
	receiptA, _ := initA.PlannedTx.Included.Eval(ctx)
	receiptB, _ := execB.PlannedTx.Included.Eval(ctx)
	receiptC, _ := initB.PlannedTx.Included.Eval(ctx)
	receiptD, _ := execA.PlannedTx.Included.Eval(ctx)

	// === Log block numbers
	t.Logf("initA:  chain %d block %d", alice.ChainID(), receiptA.BlockNumber)
	t.Logf("execB:  chain %d block %d", bob.ChainID(), receiptB.BlockNumber)
	t.Logf("initB:  chain %d block %d", bob.ChainID(), receiptC.BlockNumber)
	t.Logf("execA:  chain %d block %d", alice.ChainID(), receiptD.BlockNumber)

	// Let supervisor pick up messages
	time.Sleep(10 * time.Second)
}

// Helper to encode EventLogger.EmitLog call
func encodeEmitLogCall(data []byte) []byte {
	eventLogger := bindings.NewBindings[bindings.EventLogger]()
	calldata, err := eventLogger.EmitLog(nil, data).EncodeInputLambda()
	if err != nil {
		panic(err)
	}
	return calldata
}
