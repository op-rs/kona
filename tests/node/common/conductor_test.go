package node

import (
	"context"
	"fmt"
	"strings"
	"testing"
	"time"

	"github.com/ethereum-optimism/optimism/op-conductor/consensus"
	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-devstack/stack"
	"github.com/ethereum-optimism/optimism/op-service/testlog"
	"github.com/ethereum/go-ethereum/log"
	node_utils "github.com/op-rs/kona/node/utils"
	"github.com/stretchr/testify/require"
)

type conductorWithInfo struct {
	*dsl.Conductor
	info consensus.ServerInfo
}

// TestConductorLeadershipTransfer checks if the leadership transfer works correctly on the conductors
func TestConductorLeadershipTransfer(gt *testing.T) {
	t := devtest.SerialT(gt)
	logger := testlog.Logger(t, log.LevelInfo).With("Test", "TestConductorLeadershipTransfer")

	sys := node_utils.NewMixedOpKonaWithConductors(t)
	tracer := t.Tracer()
	ctx := t.Ctx()
	logger.Info("Started Conductor Leadership Transfer test")

	for _, conductors := range sys.ConductorSets {
		t.Gate().Greater(len(conductors), 0, "Expected at least one conductor in the system")
	}

	ctx, span := tracer.Start(ctx, "test chains")
	defer span.End()

	ctx, cancel := context.WithTimeout(ctx, 30*time.Second)
	defer cancel()

	// Test all L2 chains in the system
	for l2Chain, conductors := range sys.ConductorSets {
		chainId := l2Chain.String()

		_, span = tracer.Start(ctx, fmt.Sprintf("test chain %s", chainId))
		defer span.End()

		membership := conductors[0].FetchClusterMembership()
		require.Equal(t, len(membership.Servers), len(conductors), "cluster membership does not match the number of conductors", "chainId", chainId)

		idToConductor := make(map[string]conductorWithInfo)
		for _, conductor := range conductors {
			conductorId := strings.TrimPrefix(conductor.String(), stack.ConductorKind.String()+"-")
			idToConductor[conductorId] = conductorWithInfo{conductor, consensus.ServerInfo{}}
		}
		for _, memberInfo := range membership.Servers {
			conductor, ok := idToConductor[memberInfo.ID]
			require.True(t, ok, "unknown conductor in cluster membership", "unknown conductor id", memberInfo.ID, "chainId", chainId)
			conductor.info = memberInfo
			idToConductor[memberInfo.ID] = conductor
		}

		leaderInfo, err := conductors[0].Escape().RpcAPI().LeaderWithID(ctx)
		require.NoError(t, err, "failed to get current conductor info", "chainId", chainId)

		leaderConductor := idToConductor[leaderInfo.ID]

		voters := []conductorWithInfo{leaderConductor}
		for _, member := range membership.Servers {
			if member.ID == leaderInfo.ID || member.Suffrage == consensus.Nonvoter {
				continue
			}

			voters = append(voters, idToConductor[member.ID])
		}

		if len(voters) == 1 {
			t.Skip("only one voter found in the cluster, skipping leadership transfer test")
			continue
		}

		t.Run(fmt.Sprintf("L2_Chain_%s", chainId), func(tt devtest.T) {
			numOfLeadershipTransfers := len(voters)
			for i := range numOfLeadershipTransfers {
				oldLeaderIndex, newLeaderIndex := i%len(voters), (i+1)%len(voters)
				oldLeader, newLeader := voters[oldLeaderIndex], voters[newLeaderIndex]

				time.Sleep(3 * time.Second)

				testTransferLeadershipAndCheck(t, oldLeader, newLeader)
			}
		})
	}
}

// testTransferLeadershipAndCheck tests conductor's leadership transfer from one leader to another
func testTransferLeadershipAndCheck(t devtest.T, oldLeader, targetLeader conductorWithInfo) {
	t.Run(fmt.Sprintf("Conductor_%s_to_%s", oldLeader, targetLeader), func(tt devtest.T) {
		// ensure that the current and target leader are healthy and unpaused before transferring leadership
		require.True(tt, oldLeader.FetchSequencerHealthy(), "current leader's sequencer is not healthy, id", oldLeader)
		require.True(tt, targetLeader.FetchSequencerHealthy(), "target leader's sequencer is not healthy, id", targetLeader)
		require.False(tt, oldLeader.FetchPaused(), "current leader's sequencer is paused, id", oldLeader)
		require.False(tt, targetLeader.FetchPaused(), "target leader's sequencer is paused, id", targetLeader)

		// ensure that the current leader is the leader before transferring leadership
		require.True(tt, oldLeader.IsLeader(), "current leader was not found to be the leader")
		require.False(tt, targetLeader.IsLeader(), "target leader was already found to be the leader")

		oldLeader.TransferLeadershipTo(targetLeader.info)

		require.Eventually(
			tt,
			func() bool { return targetLeader.IsLeader() },
			5*time.Second, 1*time.Second, "target leader was not found to be the leader",
		)

		require.False(tt, oldLeader.IsLeader(), "old leader was still found to be the leader")

		// sometimes leadership transfer can cause a very brief period of unhealthiness,
		// but eventually, they should be healthy again
		require.Eventually(
			tt,
			func() bool { return oldLeader.FetchSequencerHealthy() && targetLeader.FetchSequencerHealthy() },
			3*time.Second, 1*time.Second, "at least one of the sequencers was found to be unhealthy",
		)
	})
}

// TestConductorSequencerFailover tests what happens when a sequencer fails/goes offline
func TestConductorSequencerFailover(gt *testing.T) {
	t := devtest.SerialT(gt)
	logger := testlog.Logger(t, log.LevelInfo).With("Test", "TestConductorSequencerFailover")

	sys := node_utils.NewMixedOpKonaWithConductors(t)
	tracer := t.Tracer()
	ctx := t.Ctx()
	logger.Info("Started Conductor Sequencer Failover test")

	for _, conductors := range sys.ConductorSets {
		t.Gate().Greater(len(conductors), 0, "Expected at least one conductor in the system")
	}

	ctx, span := tracer.Start(ctx, "test chains")
	defer span.End()

	ctx, cancel := context.WithTimeout(ctx, 30*time.Second)
	defer cancel()

	// Test all L2 chains in the system
	for l2Chain, conductors := range sys.ConductorSets {
		chainId := l2Chain.String()

		_, span = tracer.Start(ctx, fmt.Sprintf("test chain %s", chainId))
		defer span.End()

		if len(conductors) < 2 {
			t.Skip("need at least 2 conductors for failover test")
			continue
		}

		// Get initial leader
		leaderInfo, err := conductors[0].Escape().RpcAPI().LeaderWithID(ctx)
		require.NoError(t, err, "failed to get current leader", "chainId", chainId)

		// Find the current leader conductor
		var currentLeader *dsl.Conductor
		for _, conductor := range conductors {
			if conductor.IsLeader() {
				currentLeader = conductor
				break
			}
		}
		require.NotNil(t, currentLeader, "could not find current leader")

		// Verify the leader is healthy before stopping
		require.True(t, currentLeader.FetchSequencerHealthy(), "leader should be healthy initially")

		// Stop the current leader's sequencer (simulate failure)
		logger.Info("Stopping current leader sequencer", "leader", leaderInfo.ID)
		stopErr := currentLeader.Escape().RpcAPI().Stop(ctx)
		require.NoError(t, stopErr, "failed to stop leader sequencer")

		// Wait for leadership transfer to happen
		time.Sleep(5 * time.Second)

		// Verify a new leader was elected
		newLeaderInfo, checkErr := conductors[0].Escape().RpcAPI().LeaderWithID(ctx)
		require.NoError(t, checkErr, "failed to get new leader after failover")
		require.NotEqual(t, leaderInfo.ID, newLeaderInfo.ID, "leader should have changed after failover")

		// Verify the new leader is healthy
		for _, conductor := range conductors {
			if conductor.IsLeader() {
				require.True(t, conductor.FetchSequencerHealthy(), "new leader should be healthy")
				break
			}
		}

		logger.Info("Failover test completed successfully", "oldLeader", leaderInfo.ID, "newLeader", newLeaderInfo.ID)
	}
}

// TestConductorAddRemoveSequencer tests adding and removing a sequencer from the conductor set
func TestConductorAddRemoveSequencer(gt *testing.T) {
	t := devtest.SerialT(gt)
	logger := testlog.Logger(t, log.LevelInfo).With("Test", "TestConductorAddRemoveSequencer")

	sys := node_utils.NewMixedOpKonaWithConductors(t)
	tracer := t.Tracer()
	ctx := t.Ctx()
	logger.Info("Started Conductor Add/Remove Sequencer test")

	for _, conductors := range sys.ConductorSets {
		t.Gate().Greater(len(conductors), 0, "Expected at least one conductor in the system")
	}

	ctx, span := tracer.Start(ctx, "test chains")
	defer span.End()

	ctx, cancel := context.WithTimeout(ctx, 45*time.Second)
	defer cancel()

	// Test all L2 chains in the system
	for l2Chain, conductors := range sys.ConductorSets {
		chainId := l2Chain.String()

		_, span = tracer.Start(ctx, fmt.Sprintf("test chain %s", chainId))
		defer span.End()

		if len(conductors) < 2 {
			t.Skip("need at least 2 conductors for add/remove test")
			continue
		}

		// Get initial cluster membership
		initialMembership := conductors[0].FetchClusterMembership()
		initialMemberCount := len(initialMembership.Servers)
		logger.Info("Initial cluster membership", "count", initialMemberCount)

		// Find a non-leader sequencer to pause/remove
		var targetSequencer *dsl.Conductor
		for _, conductor := range conductors {
			if !conductor.IsLeader() {
				targetSequencer = conductor
				break
			}
		}

		if targetSequencer == nil {
			t.Skip("could not find a non-leader sequencer to test")
			continue
		}

		// Pause the sequencer (simulate removal)
		logger.Info("Pausing sequencer", "sequencer", targetSequencer.String())
		err := targetSequencer.Escape().RpcAPI().Pause(ctx)
		require.NoError(t, err, "failed to pause sequencer")

		// Verify sequencer is paused
		require.Eventually(t, func() bool {
			return targetSequencer.FetchPaused()
		}, 5*time.Second, 1*time.Second, "sequencer should be paused")

		// Wait a bit to ensure cluster stabilizes
		time.Sleep(3 * time.Second)

		// Resume the sequencer (simulate re-adding)
		logger.Info("Resuming sequencer", "sequencer", targetSequencer.String())
		err = targetSequencer.Escape().RpcAPI().Resume(ctx)
		require.NoError(t, err, "failed to resume sequencer")

		// Verify sequencer is unpaused and healthy
		require.Eventually(t, func() bool {
			return !targetSequencer.FetchPaused() && targetSequencer.FetchSequencerHealthy()
		}, 10*time.Second, 1*time.Second, "sequencer should be resumed and healthy")

		// Verify cluster membership is stable
		finalMembership := conductors[0].FetchClusterMembership()
		require.Equal(t, initialMemberCount, len(finalMembership.Servers), "cluster membership should be stable")

		logger.Info("Add/Remove sequencer test completed successfully")
	}
}

// TestConductorInitialSync tests initial conductor synchronization when joining the cluster
func TestConductorInitialSync(gt *testing.T) {
	t := devtest.SerialT(gt)
	logger := testlog.Logger(t, log.LevelInfo).With("Test", "TestConductorInitialSync")

	sys := node_utils.NewMixedOpKonaWithConductors(t)
	tracer := t.Tracer()
	ctx := t.Ctx()
	logger.Info("Started Conductor Initial Sync test")

	for _, conductors := range sys.ConductorSets {
		t.Gate().Greater(len(conductors), 0, "Expected at least one conductor in the system")
	}

	ctx, span := tracer.Start(ctx, "test chains")
	defer span.End()

	ctx, cancel := context.WithTimeout(ctx, 30*time.Second)
	defer cancel()

	// Test all L2 chains in the system
	for l2Chain, conductors := range sys.ConductorSets {
		chainId := l2Chain.String()

		_, span = tracer.Start(ctx, fmt.Sprintf("test chain %s", chainId))
		defer span.End()

		// Verify all conductors can fetch cluster membership
		for i, conductor := range conductors {
			membership := conductor.FetchClusterMembership()
			require.NotNil(t, membership, "conductor should be able to fetch cluster membership", "index", i)
			require.Greater(t, len(membership.Servers), 0, "cluster should have at least one member", "index", i)
			logger.Info("Conductor synced successfully", "index", i, "members", len(membership.Servers))
		}

		// Verify all conductors agree on cluster membership
		firstMembership := conductors[0].FetchClusterMembership()
		for i := 1; i < len(conductors); i++ {
			otherMembership := conductors[i].FetchClusterMembership()
			require.Equal(t, len(firstMembership.Servers), len(otherMembership.Servers),
				"all conductors should agree on cluster size", "conductor", i)
		}

		// Verify all conductors are healthy
		for i, conductor := range conductors {
			require.True(t, conductor.FetchSequencerHealthy(),
				"conductor sequencer should be healthy after sync", "index", i)
			logger.Info("Conductor health verified", "index", i)
		}

		// Verify there is exactly one leader
		leaderCount := 0
		for _, conductor := range conductors {
			if conductor.IsLeader() {
				leaderCount++
			}
		}
		require.Equal(t, 1, leaderCount, "cluster should have exactly one leader")

		logger.Info("Initial sync test completed successfully", "chainId", chainId)
	}
}
