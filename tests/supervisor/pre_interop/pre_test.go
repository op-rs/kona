package preinterop

import (
	"math/rand"
	"testing"
	"time"

	"github.com/ethereum/go-ethereum/crypto"

	"github.com/ethereum-optimism/optimism/op-acceptance-tests/tests/interop"
	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-devstack/presets"
	"github.com/ethereum-optimism/optimism/op-service/eth"
	stypes "github.com/ethereum-optimism/optimism/op-supervisor/supervisor/types"
	gethTypes "github.com/ethereum/go-ethereum/core/types"
)

func TestPreInteropNoSyncStatus(gt *testing.T) {
	t := devtest.ParallelT(gt)
	sys := presets.NewSimpleInterop(t)
	require := t.Require()

	t.Logger().Info("Starting")

	devtest.RunParallel(t, sys.L2Networks(), func(t devtest.T, net *dsl.L2Network) {
		interopTime := net.Escape().ChainConfig().InteropTime
		t.Require().NotNil(interopTime)

		require.False(net.IsActivated(*interopTime), "Test requires pre-interop state")
		t.Logger().Info("Timestamps", "interopTime", *interopTime, "now", time.Now().Unix())

		_, err := sys.Supervisor.Escape().QueryAPI().SyncStatus(t.Ctx())
		require.ErrorContains(err, "chain database is not initialized")
	})

	t.Logger().Info("Done")
}

func TestPreInteropCheckAccessList(gt *testing.T) {
	t := devtest.ParallelT(gt)
	sys := presets.NewSimpleInterop(t)
	require := t.Require()

	alice := sys.FunderA.NewFundedEOA(eth.OneHundredthEther)
	bob := sys.FunderB.NewFundedEOA(eth.OneHundredthEther)

	eventLoggerAddress := alice.DeployEventLogger()
	sys.L2ChainB.CatchUpTo(sys.L2ChainA)

	rng := rand.New(rand.NewSource(time.Now().UnixNano()))
	_, initReceipt := alice.SendInitMessage(
		interop.RandomInitTrigger(rng, eventLoggerAddress, rng.Intn(3), rng.Intn(10)),
	)

	logToAccess := func(chainID eth.ChainID, log *gethTypes.Log, timestamp uint64) stypes.Access {
		msgPayload := make([]byte, 0)
		for _, topic := range log.Topics {
			msgPayload = append(msgPayload, topic.Bytes()...)
		}
		msgPayload = append(msgPayload, log.Data...)

		msgHash := crypto.Keccak256Hash(msgPayload)
		args := stypes.ChecksumArgs{
			BlockNumber: log.BlockNumber,
			Timestamp:   timestamp,
			LogIndex:    uint32(log.Index),
			ChainID:     chainID,
			LogHash:     stypes.PayloadHashToLogHash(msgHash, log.Address),
		}
		return args.Access()
	}

	blockRef := sys.L2ChainA.PublicRPC().BlockRefByNumber(initReceipt.BlockNumber.Uint64())

	var accessEntries []stypes.Access
	for _, evLog := range initReceipt.Logs {
		accessEntries = append(accessEntries, logToAccess(alice.ChainID(), evLog, blockRef.Time))
	}

	sys.L2ChainB.WaitForBlock()

	accessList := stypes.EncodeAccessList(accessEntries)
	timestamp := uint64(time.Now().Unix())
	ed := stypes.ExecutingDescriptor{
		Timestamp: timestamp,
		ChainID:   bob.ChainID(),
	}

	err := sys.Supervisor.Escape().QueryAPI().CheckAccessList(t.Ctx(), accessList, stypes.LocalUnsafe, ed)
	require.Error(err, "CheckAccessList should fail before interop")
}
